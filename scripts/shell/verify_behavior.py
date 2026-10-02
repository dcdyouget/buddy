#!/usr/bin/env python3
"""S07-03/04 系统热键与隐藏拦截；运行期间禁止其他源码编辑或 cargo。

每项只改一个锚点，finally 恢复原始字节。编译失败、权限失败、超时不计拦截。
"""

import argparse
from pathlib import Path
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "settings"))
from verify_phase06 import Case, ROOT, run

CORE = "crates/ui/src/shell/hotkey/core.rs"
RUNTIME = "crates/ui/src/shell/runtime.rs"
VISIBILITY = "crates/ui/src/shell/visibility.rs"
SELECTION = "crates/ui/src/shell/selection.rs"
PREFERENCES = "crates/ui/src/chat/router_preferences.rs"
UNIT = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib", "shell::")
GUI = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "shell_preview", "--", "--selftest-behavior")
FAIL = "FAIL S07-03/S07-04"

CASES = [
    Case("selection-before-activation", RUNTIME, "            let capture = selection::begin_before_show(cx);", "            let _ = show(handle, cx).await;\n            let capture = selection::begin_before_show(cx);", GUI, FAIL),
    Case("selection-publish", RUNTIME, "router.accept_selected_text(&text, cx)", "{ let _ = (router, text, cx); }", GUI, FAIL),
    Case("streaming-focus-bubble", "crates/ui/src/chat/composer.rs", "                .track_focus(&self.text.focus_handle(cx))", "", GUI, FAIL),
    Case("os-exclusive-registration", "vendor/global-hotkey/src/platform_impl/macos/mod.rs", "                    K_EVENT_HOT_KEY_EXCLUSIVE,", "                    0,", GUI, FAIL),
    Case("press-dedup", CORE, "HotKeyState::Pressed => self.pressed.insert(event.id),", "HotKeyState::Pressed => { self.pressed.insert(event.id); true },", UNIT, "FAILED"),
    Case("release-no-toggle", CORE, "self.pressed.remove(&event.id);\n                false", "self.pressed.remove(&event.id);\n                true", UNIT, "FAILED"),
    Case("current-id", CORE, "if self.current.is_none_or(|hotkey| hotkey.id() != event.id) {", "if false {", UNIT, "FAILED"),
    Case("unchanged-key", CORE, "if self.current == Some(hotkey) {", "if false {", UNIT, "FAILED"),
    Case("cleanup-before-update", CORE, "self.cleanup_except(old)?;", "let _ = old;", UNIT, "FAILED"),
    Case("rollback-new-registration", CORE, "match self.backend.unregister(hotkey) {", "match Ok::<(), String>(()) {", UNIT, "FAILED"),
    Case("unregister-old", CORE, "if let Err(old_reason) = self.backend.unregister(old) {", "if let Err(old_reason) = self.backend.unregister(hotkey) {", UNIT, "FAILED"),
    Case("selection-trim", SELECTION, "Some(trimmed.to_owned())", "Some(candidate.to_owned())", UNIT, "FAILED"),
    Case("selection-unchanged", SELECTION, "candidate.is_empty() || candidate == previous", "candidate.is_empty() || false", UNIT, "FAILED"),
    Case("selection-clipboard-restore", SELECTION, "cx.write_to_clipboard(restore);", "let _ = restore;", GUI, FAIL),
    Case("idle-compact", RUNTIME, "idle >= Duration::from_secs(600)", "idle >= Duration::from_secs(601)", UNIT, "FAILED"),
    Case("hotkey-channel", RUNTIME, "let accepted = event_manager.borrow_mut().accept(event);", "let accepted = false; let _ = (&event_manager, event);", GUI, FAIL),
    Case("three-state-toggle", RUNTIME, "if snapshot.is_visible && snapshot.is_key {", "if snapshot.is_visible {", GUI, FAIL),
    Case("native-hide", VISIBILITY, "orderOut: std::ptr::null_mut::<objc::runtime::Object>()", "orderFront: std::ptr::null_mut::<objc::runtime::Object>()", GUI, FAIL),
    Case("outside-click-mask", VISIBILITY, "let mask = LEFT_MOUSE_DOWN | RIGHT_MOUSE_DOWN | OTHER_MOUSE_DOWN;", "let mask = 0usize;", GUI, FAIL),
    Case("outside-click-delivery", VISIBILITY, "let _ = sender.send(VisibilityEvent::ExternalMouseDown);", "let _ = &sender;", GUI, FAIL),
    Case("esc-hide", "crates/ui/src/shell/mod.rs", "runtime::request_hide(handle, cx);", "let _ = (handle, cx);", GUI, FAIL),
    Case("hotkey-save-registration", PREFERENCES, "Some(update) => update(&candidate.hotkey),", "Some(_) => Ok(()),", GUI, FAIL),
    Case("hotkey-save-rollback", PREFERENCES, "Some(update) => update(&baseline.hotkey),", "Some(_) => Ok::<(), String>(()),", GUI, FAIL),
    Case("approval-esc-priority", "crates/ui/src/chat/chat_page.rs", "cx.stop_propagation();\n                    decide(id, Decision::Deny, cx);", "decide(id, Decision::Deny, cx);", GUI, FAIL),
    Case("hidden-does-not-cancel", "crates/ui/src/chat/router.rs", "self.window_visibility_changed(false, cx);", "self.engine.stop_generation(); self.window_visibility_changed(false, cx);", GUI, FAIL),
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--case", action="append", default=[])
    args = parser.parse_args()
    known = {case.name for case in CASES}
    if set(args.case) - known:
        parser.error("未知拦截项")
    if args.list:
        for case in CASES:
            print(f"{case.name}: {case.path}")
        return 0
    cases = [case for case in CASES if not args.case or case.name in args.case]
    logs = Path(tempfile.mkdtemp(prefix="buddy-s07-behavior-interception-"))
    print(f"日志目录：{logs}", flush=True)
    for index, command in enumerate(dict.fromkeys(case.command for case in cases)):
        rc, output = run(command, logs / f"baseline-{index}.log")
        marker = "PASS S07-03/S07-04" if command == GUI else "test result: ok."
        if rc or marker not in output or "running 0 tests" in output:
            print(f"基线失败 rc={rc}：{output}", flush=True)
            return 1
    results = []
    for case in cases:
        path = ROOT / case.path
        original = path.read_bytes()
        source = original.decode()
        if source.count(case.before) != 1:
            print(f"FAIL {case.name}: 锚点不唯一，拒绝修改", flush=True)
            results.append(False)
            continue
        try:
            path.write_text(source.replace(case.before, case.after, 1))
            rc, output = run(case.command, logs / f"{case.name}.log")
        finally:
            path.write_bytes(original)
        intercepted = rc not in (0, 124) and case.fail_marker in output and "error[E" not in output and "could not compile" not in output and not any(marker in output for marker in ("权限预检未通过", "会话已锁定", "未获得 CGEventPost 权限"))
        results.append(intercepted)
        print(f"{'PASS' if intercepted else 'FAIL'} {case.name}: rc={rc}; 源码已还原", flush=True)
        if not intercepted:
            print(output, flush=True)
    print(f"拦截 {sum(results)}/{len(results)}；日志：{logs}", flush=True)
    return 0 if all(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
