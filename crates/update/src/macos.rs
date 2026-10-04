//! macOS 安装：解压已验证的 `Buddy.app` → 校验 → 在原位置同目录内改名替换。
//!
//! 替换顺序（目录写权限足够时）：
//!   1. `ditto` 复制新包到 `.<名称>.update-new`（与目标同目录，保证之后的 rename 是原子的）
//!   2. 目标改名为 `.<名称>.update-old`
//!   3. 新包改名为目标；失败则把旧包改名回来
//!   4. 删除旧包（运行中的进程已映射文件，删除不影响本次退出）
//! 目录不可写（如 root 拥有的 `/Applications`）时，同一序列经 osascript 管理员授权执行。

use crate::{BUNDLE_ID, Installed, UpdateError, file_name};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(crate) fn install(archive: &Path, version: &str) -> Result<Installed, UpdateError> {
    let target = current_app()?;
    let staging = archive
        .parent()
        .ok_or_else(|| UpdateError::Install("更新包路径无效".into()))?
        .join("staging");
    if staging.exists() {
        std::fs::remove_dir_all(&staging).map_err(install_error("清理解压目录失败"))?;
    }
    std::fs::create_dir_all(&staging).map_err(install_error("创建解压目录失败"))?;
    run(
        Command::new("/usr/bin/tar")
            .arg("-xzf")
            .arg(archive)
            .arg("-C")
            .arg(&staging),
        "解压更新包失败",
    )?;
    let new_app = single_app(&staging)?;
    validate_bundle(&new_app, version)?;
    swap(&new_app, &target)?;
    Ok(Installed { app: target })
}

/// 当前运行的 `.app`；开发构建、从 DMG 直接运行或被系统随机化路径运行时拒绝更新。
pub(crate) fn current_app() -> Result<PathBuf, UpdateError> {
    let exe = std::env::current_exe()
        .and_then(|p| p.canonicalize())
        .map_err(install_error("无法定位当前应用"))?;
    let app = exe
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .filter(|p| p.extension().is_some_and(|e| e == "app"))
        .ok_or_else(|| UpdateError::Install("开发构建不支持自动更新，请使用安装包运行".into()))?;
    let text = app.to_string_lossy();
    // App Translocation：未移出下载目录的隔离应用运行在只读的随机路径；DMG 卷同样只读。
    if text.contains("/AppTranslocation/") || text.starts_with("/Volumes/") {
        return Err(UpdateError::Install(
            "请先将 Buddy 拖入「应用程序」文件夹并从那里打开，再检查更新".into(),
        ));
    }
    Ok(app.to_path_buf())
}

fn single_app(staging: &Path) -> Result<PathBuf, UpdateError> {
    let apps: Vec<PathBuf> = std::fs::read_dir(staging)
        .map_err(install_error("读取解压目录失败"))?
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "app"))
        .collect();
    match apps.as_slice() {
        [app] => Ok(app.clone()),
        _ => Err(UpdateError::Install("更新包内容无效：应当只包含一个应用".into())),
    }
}

/// 新包必须是 Buddy、版本与清单一致、可执行文件存在且代码签名完整。
pub(crate) fn validate_bundle(app: &Path, version: &str) -> Result<(), UpdateError> {
    let plist = app.join("Contents/Info.plist");
    let id = plist_value(&plist, "CFBundleIdentifier")?;
    if id != BUNDLE_ID {
        return Err(UpdateError::Install(format!("更新包不是 Buddy（{id}）")));
    }
    let actual = plist_value(&plist, "CFBundleShortVersionString")?;
    if actual != version {
        return Err(UpdateError::Install(format!(
            "更新包版本 {actual} 与清单 {version} 不一致"
        )));
    }
    let executable = plist_value(&plist, "CFBundleExecutable")?;
    if !app.join("Contents/MacOS").join(&executable).is_file() {
        return Err(UpdateError::Install("更新包缺少可执行文件".into()));
    }
    run(
        Command::new("/usr/bin/codesign")
            .args(["--verify", "--deep", "--strict"])
            .arg(app),
        "更新包代码签名校验失败",
    )
}

fn plist_value(plist: &Path, key: &str) -> Result<String, UpdateError> {
    let output = Command::new("/usr/bin/plutil")
        .args(["-extract", key, "raw", "-o", "-"])
        .arg(plist)
        .output()
        .map_err(install_error("读取更新包信息失败"))?;
    if !output.status.success() {
        return Err(UpdateError::Install(format!("更新包缺少 {key}")));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn sibling(target: &Path, suffix: &str) -> PathBuf {
    target.with_file_name(format!(".{}.{suffix}", file_name(target)))
}

/// 用 `new_app` 原子替换 `target`；目录不可写时请求管理员授权。
pub(crate) fn swap(new_app: &Path, target: &Path) -> Result<(), UpdateError> {
    let staged = sibling(target, "update-new");
    let backup = sibling(target, "update-old");
    for leftover in [&staged, &backup] {
        if leftover.exists() {
            let _ = std::fs::remove_dir_all(leftover);
        }
    }
    // 只有「目录不可写」才走管理员授权；其他失败（磁盘满等）直接报错。
    match std::fs::create_dir(&staged) {
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return swap_privileged(new_app, target, &staged, &backup);
        }
        Err(e) => return Err(UpdateError::Install(format!("准备新版本失败：{e}"))),
        Ok(()) => {}
    }
    // 目标目录已存在时 ditto 把源目录内容复制进去
    if let Err(error) = run(
        Command::new("/usr/bin/ditto").arg(new_app).arg(&staged),
        "复制新版本失败",
    ) {
        let _ = std::fs::remove_dir_all(&staged);
        return Err(error);
    }
    std::fs::rename(target, &backup).map_err(|e| {
        let _ = std::fs::remove_dir_all(&staged);
        UpdateError::Install(format!("替换应用失败：{e}"))
    })?;
    if let Err(error) = std::fs::rename(&staged, target) {
        // 回滚：旧应用原样放回，用户不会得到损坏的安装
        let restored = std::fs::rename(&backup, target);
        let _ = std::fs::remove_dir_all(&staged);
        return Err(UpdateError::Install(match restored {
            Ok(()) => format!("替换应用失败，已恢复原版本：{error}"),
            Err(e) => format!("替换应用失败且恢复失败：{error}；{e}（备份位于 {}）", backup.display()),
        }));
    }
    if let Err(error) = std::fs::remove_dir_all(&backup) {
        log::warn!("删除旧应用备份失败，下次启动再清理：{error}");
    }
    Ok(())
}

fn swap_privileged(
    new_app: &Path,
    target: &Path,
    staged: &Path,
    backup: &Path,
) -> Result<(), UpdateError> {
    let [src, dst, new, old] = [new_app, target, staged, backup].map(shell_quote);
    let script = format!(
        "/bin/rm -rf {new} {old} && /usr/bin/ditto {src} {new} && /bin/mv {dst} {old} && \
         if /bin/mv {new} {dst}; then /bin/rm -rf {old}; else /bin/mv {old} {dst}; /bin/rm -rf {new}; exit 1; fi"
    );
    let apple = format!(
        "do shell script \"{}\" with administrator privileges with prompt \"Buddy 需要授权以完成更新\"",
        script.replace('\\', "\\\\").replace('"', "\\\"")
    );
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", &apple])
        .output()
        .map_err(install_error("请求管理员授权失败"))?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    Err(UpdateError::Install(if stderr.contains("-128") {
        "已取消授权，更新未安装".into()
    } else {
        format!("以管理员身份替换应用失败：{}", stderr.trim())
    }))
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path.to_string_lossy().replace('\'', "'\\''"))
}

pub(crate) fn relaunch_after_exit(app: &Path) -> Result<(), UpdateError> {
    // 最多等 60 秒；超时仍 open（若旧实例还在，单实例转发只会把它唤起，不会多开）。
    Command::new("/bin/sh")
        .arg("-c")
        .arg(
            "i=0; while /bin/kill -0 \"$1\" 2>/dev/null && [ $i -lt 300 ]; do /bin/sleep 0.2; i=$((i+1)); done; exec /usr/bin/open \"$2\"",
        )
        .arg("sh")
        .arg(std::process::id().to_string())
        .arg(app)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(install_error("启动新版本失败"))
}

pub(crate) fn cleanup_backups() {
    let Ok(app) = current_app() else {
        return;
    };
    for leftover in [sibling(&app, "update-old"), sibling(&app, "update-new")] {
        if leftover.exists()
            && let Err(error) = std::fs::remove_dir_all(&leftover)
        {
            log::warn!("清理 {} 失败：{error}", leftover.display());
        }
    }
}

fn run(command: &mut Command, message: &str) -> Result<(), UpdateError> {
    let output = command.output().map_err(install_error(message))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(UpdateError::Install(format!(
            "{message}：{}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }
}

fn install_error(message: &str) -> impl Fn(std::io::Error) -> UpdateError + '_ {
    move |e| UpdateError::Install(format!("{message}：{e}"))
}

#[cfg(test)]
#[path = "macos_tests.rs"]
mod tests;
