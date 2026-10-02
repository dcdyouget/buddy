//! T47：真实 macOS 全局热键切换、旧键失效和流式隐藏重显。

use super::{external_target, fixture, hotkey_owner, input, os_input};
use buddy_ui::chat::page_state::Page;
use buddy_ui::chat_bridge::spawn_engine;
use buddy_ui::gpui::AsyncApp;
use buddy_ui::shell;
use std::time::Duration;

const OLD_KEY: &str = "B";
const NEW_KEY: &str = "N";
const OLD_CONFIG: &str = "CmdOrCtrl+Alt+Shift+B";
const NEW_CONFIG: &str = "CmdOrCtrl+Alt+Shift+N";

#[path = "t47_support.rs"]
mod support;

use support::{
    block_engine_directory, config_with_hotkey, equivalent_hotkey, press_global, probe,
    record_hotkey, restore_engine_directory, router_of, visible, wait_file, wait_shown_and_focused,
    wait_streaming, wait_streaming_without_draw,
};

pub(crate) async fn run(cx: &mut AsyncApp) -> bool {
    macro_rules! fail {
        ($message:expr) => {{
            println!("FAIL T47：{}", $message);
            return false;
        }};
    }

    let preflight = os_input::preflight();
    if preflight != os_input::Preflight::Ready {
        println!("FAIL T47：{}", os_input::preflight_message(preflight));
        return false;
    }
    let engine = fixture::manual_engine(false);
    if let Err(error) = config_with_hotkey(&engine, OLD_CONFIG, cx).await {
        fail!(format!("写入预览热键失败：{error}"));
    }
    let handle = match shell::open_main_window(engine.clone(), fixture::shell_config(), cx).await {
        Ok(handle) => handle,
        Err(error) => fail!(format!("打开主窗口失败：{error}")),
    };
    input::settle(handle, cx).await;
    let Some(router) = router_of(handle, cx) else {
        fail!("读取 Router 失败");
    };
    let initial = visible(handle, cx) == Some(true);
    let installed = shell::runtime::install(handle, cx).await.is_ok();
    let shown = shell::runtime::show(handle, cx).await.is_ok();
    cx.update(|app| app.activate(true));
    input::draw(handle, cx).await;
    let immediate_focus = probe(handle, cx);
    println!(
        "T47 初始显示即时状态：visible={} key={}",
        immediate_focus
            .as_ref()
            .is_some_and(|snapshot| snapshot.is_visible),
        immediate_focus
            .as_ref()
            .is_some_and(|snapshot| snapshot.is_key),
    );
    let final_focus = if shown {
        wait_shown_and_focused(handle, cx).await
    } else {
        None
    };
    let shown_and_focused = final_focus.is_some();
    println!(
        "T47 初始显示最终状态：visible={} key={}（等待上限 2s）",
        final_focus
            .as_ref()
            .is_some_and(|snapshot| snapshot.is_visible),
        final_focus.as_ref().is_some_and(|snapshot| snapshot.is_key),
    );

    // 按下与释放分别注入；释放阶段不得再次切换。
    let old_hide = press_global(handle, OLD_KEY, false, cx).await;
    let old_show = press_global(handle, OLD_KEY, true, cx).await;

    let config_updated = record_hotkey(handle, &router, "n", NEW_CONFIG, cx).await;
    let baseline_hotkey = router.read_with(cx, |router, _| router.config().hotkey.clone());
    let baseline_registered = cx
        .update(|app| shell::runtime::registered_hotkey(app))
        .unwrap_or_default();
    let registered = baseline_registered.contains("N");
    let _ = router.update(cx, |router, cx| router.close_settings(cx));
    let old_ignored = os_input::combo(OLD_KEY);
    cx.background_executor()
        .timer(Duration::from_millis(150))
        .await;
    let old_ignored = old_ignored && visible(handle, cx) == Some(true);
    let mut unfocused_child = external_target::spawn().ok();
    let unfocused = if let Some(target) = unfocused_child.as_ref() {
        wait_file(&target.ready, cx).await
            && probe(handle, cx).is_some_and(|snapshot| snapshot.is_visible && !snapshot.is_key)
    } else {
        false
    };
    let refocused = if unfocused {
        press_global(handle, NEW_KEY, true, cx).await
            && probe(handle, cx).is_some_and(|snapshot| snapshot.is_visible && snapshot.is_key)
    } else {
        false
    };
    if let Some(target) = unfocused_child.take() {
        drop(target);
    }
    let new_hide = press_global(handle, NEW_KEY, false, cx).await;
    let new_show = press_global(handle, NEW_KEY, true, cx).await;

    // A separate process owns the candidate: this exercises actual OS registration conflict.
    let owner = match hotkey_owner::spawn() {
        Ok(owner) => owner,
        Err(error) => fail!(format!("启动热键占用进程失败：{error}")),
    };
    let owner_ready = wait_file(&owner.ready, cx).await;
    let conflict_attempt =
        owner_ready && !record_hotkey(handle, &router, "b", OLD_CONFIG, cx).await;
    let conflict_error = router.read_with(cx, |router, app| {
        router
            .settings_view()
            .read(app)
            .hotkey_recorder()
            .read(app)
            .error()
            .is_some_and(|message| message.contains("注册全局热键") && message.contains("失败"))
    });
    let conflict_memory =
        router.read_with(cx, |router, _| router.config().hotkey == baseline_hotkey);
    let conflict_registration = cx
        .update(|app| shell::runtime::registered_hotkey(app))
        .as_deref()
        == Some(baseline_registered.as_str());
    let conflict_disk = {
        let engine = engine.clone();
        cx.update(|app| spawn_engine(app, async move { engine.get_config().await }))
            .await
            .is_ok_and(|config| config.hotkey == baseline_hotkey)
    };
    let owner_receives = owner_ready && os_input::combo(OLD_KEY) && wait_file(&owner.ack, cx).await;
    let _ = router.update(cx, |router, cx| router.close_settings(cx));
    let old_still_works = press_global(handle, NEW_KEY, false, cx).await
        && press_global(handle, NEW_KEY, true, cx).await;
    let conflict_preserved = conflict_attempt
        && conflict_error
        && conflict_memory
        && conflict_registration
        && conflict_disk
        && owner_receives
        && old_still_works;
    println!(
        "T47 OS冲突：ready/失败/错误/内存/注册/磁盘/owner ACK/旧注册仍可切换 {owner_ready}/{conflict_attempt}/{conflict_error}/{conflict_memory}/{conflict_registration}/{conflict_disk}/{owner_receives}/{old_still_works}"
    );
    drop(owner);

    let Some(backup) = block_engine_directory(&engine) else {
        fail!("无法构造真实保存失败沙盒");
    };
    let failure_attempt = !record_hotkey(handle, &router, "b", OLD_CONFIG, cx).await;
    let failure_error = router.read_with(cx, |router, app| {
        router
            .settings_view()
            .read(app)
            .hotkey_recorder()
            .read(app)
            .error()
            .is_some_and(|message| message.contains("保存快捷键失败"))
    });
    let memory_value = router.read_with(cx, |router, _| router.config().hotkey.clone());
    let recorder_value = router.read_with(cx, |router, app| {
        router
            .settings_view()
            .read(app)
            .hotkey_recorder()
            .read(app)
            .current()
            .to_owned()
    });
    let registered_value = cx
        .update(|app| shell::runtime::registered_hotkey(app))
        .unwrap_or_else(|| "<无注册热键>".to_owned());
    let memory_kept = memory_value == baseline_hotkey;
    let registered_kept = equivalent_hotkey(&registered_value, &baseline_registered);
    let restored = restore_engine_directory(&engine, backup);
    let disk_result = if restored {
        let read_engine = engine.clone();
        match cx
            .update(|app| spawn_engine(app, async move { read_engine.get_config().await }))
            .await
        {
            Ok(config) => format!("{}", config.hotkey),
            Err(error) => format!("<读取失败: {error}>"),
        }
    } else {
        "<恢复目录失败>".to_owned()
    };
    let disk_kept = disk_result == baseline_hotkey;
    println!(
        "T47 失败回滚诊断：候选尝试失败={}；错误={}；Router 热键={:?}；录制器={:?}；已注册={:?}；磁盘={:?}；目录恢复={}",
        failure_attempt,
        failure_error,
        memory_value,
        recorder_value,
        registered_value,
        disk_result,
        restored,
    );
    let _ = router.update(cx, |router, cx| router.close_settings(cx));
    let failure_recovery =
        failure_attempt && failure_error && memory_kept && registered_kept && disk_kept && restored;

    let same_router = router_of(handle, cx).is_some_and(|current| current == router);
    let pasted = input::paste_text(handle, "慢", cx).await;
    input::press(handle, "enter", cx).await;
    let streaming = wait_streaming(handle, true, cx).await;
    let hidden_stream = press_global(handle, NEW_KEY, false, cx).await;
    let completed_hidden = wait_streaming_without_draw(handle, false, cx).await;
    let reopened = press_global(handle, NEW_KEY, true, cx).await;
    let complete_page = handle
        .read_with(cx, |shell, app| {
            shell.router().read(app).page() == Page::Conversation
        })
        .unwrap_or(false);
    let complete_text = handle
        .read_with(cx, |shell, app| {
            shell
                .router()
                .read(app)
                .conversation()
                .read(app)
                .state
                .messages
                .last()
                .is_some_and(|message| crate::fixture::complete_slow_response(&message.content))
        })
        .unwrap_or(false);

    let ok = initial
        && installed
        && shown_and_focused
        && old_hide
        && old_show
        && config_updated
        && registered
        && old_ignored
        && unfocused
        && refocused
        && new_hide
        && new_show
        && conflict_preserved
        && failure_recovery
        && same_router
        && pasted
        && streaming
        && hidden_stream
        && completed_hidden
        && reopened
        && complete_page
        && complete_text;
    println!(
        "T47: 权限/初始/安装/聚焦 {}/{}/{}/{}；旧键隐藏/重显 {}/{}；更新/注册/旧键失效 {}/{}/{}；失焦/热键唤回 {}/{}；新键隐藏/重显 {}/{}；保存失败恢复 {}/{}/{}/{}/{}/{}/{}；同 Router/输入/流式 {}/{}/{}；隐藏/完成/重开/终态 {}/{}/{}/{}/{}",
        os_input::can_post(),
        initial,
        installed,
        shown_and_focused,
        old_hide,
        old_show,
        config_updated,
        registered,
        old_ignored,
        unfocused,
        refocused,
        new_hide,
        new_show,
        failure_attempt,
        failure_error,
        memory_kept,
        registered_kept,
        disk_kept,
        restored,
        failure_recovery,
        same_router,
        pasted,
        streaming,
        hidden_stream,
        completed_hidden,
        reopened,
        complete_page,
        complete_text,
    );
    ok
}
