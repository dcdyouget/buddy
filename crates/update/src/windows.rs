//! Windows 安装：已验签的原始 PE 先放到当前 `buddy.exe` 同目录，当前进程
//! 退出后由独立 PowerShell 进程完成替换并重新启动。
//!
//! Windows 不允许替换正在运行的 `.exe`。因此 [`install`] 只执行可回滚的暂存，
//! [`relaunch_after_exit`] 才注册一个等待当前 PID 的替换器。发布制品是
//! `Buddy_<version>_x86_64.exe`，不接受 zip 或安装器作为应用内更新包。

use crate::{Installed, UpdateError, file_name};
use std::os::windows::process::CommandExt;
use std::{
    ffi::OsStr,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
};

const STAGED_SUFFIX: &str = "update-new";
const BACKUP_SUFFIX: &str = "update-old";
const SCRIPT_SUFFIX: &str = "update.ps1";
// CREATE_NO_WINDOW：替换器是后台进程，不应在更新时短暂闪出控制台窗口。
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 暂存已验签的更新包。版本号已经在清单中经 semver 检查，Windows PE 中没有
/// 可移植、可信的版本字段可用于再一次比对，因此完整性和来源以下载阶段的
/// sha256 + minisign 校验为准。
pub(crate) fn install(archive: &Path, _version: &str) -> Result<Installed, UpdateError> {
    let target = current_app()?;
    validate_update_executable(archive)?;
    stage_update(archive, &target)?;
    Ok(Installed { app: target })
}

/// 仅允许从已安装的 `buddy.exe` 自更新。排除 Cargo 的 `target/` 输出，避免
/// 开发构建覆盖自身；便携版只要不是构建输出，可放在任意用户可写目录。
pub(crate) fn current_app() -> Result<PathBuf, UpdateError> {
    let exe = std::env::current_exe()
        .and_then(|path| path.canonicalize())
        .map_err(install_error("无法定位当前应用"))?;
    current_app_from(&exe)
}

fn current_app_from(exe: &Path) -> Result<PathBuf, UpdateError> {
    if !is_buddy_executable(exe) || has_target_component(exe) {
        return Err(UpdateError::Install(
            "开发构建不支持自动更新，请使用已安装的 Buddy".into(),
        ));
    }
    Ok(exe.to_path_buf())
}

fn is_buddy_executable(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("buddy.exe"))
        && path
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
}

fn has_target_component(path: &Path) -> bool {
    path.components().any(|component| match component {
        Component::Normal(name) => name.eq_ignore_ascii_case("target"),
        _ => false,
    })
}

fn stage_update(archive: &Path, target: &Path) -> Result<(), UpdateError> {
    let staged = sibling(target, STAGED_SUFFIX);
    remove_regular_file_if_exists(&staged, "清理上次暂存的更新失败")?;
    fs::copy(archive, &staged).map_err(install_error("暂存新版本失败"))?;
    Ok(())
}

fn validate_update_executable(archive: &Path) -> Result<(), UpdateError> {
    if !archive
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
    {
        return Err(UpdateError::Install(
            "Windows 更新包必须是 .exe 文件".into(),
        ));
    }
    let metadata = fs::metadata(archive).map_err(install_error("读取更新包失败"))?;
    if !metadata.is_file() {
        return Err(UpdateError::Install("Windows 更新包不是普通文件".into()));
    }
    let mut header = [0_u8; 2];
    fs::File::open(archive)
        .and_then(|mut file| file.read_exact(&mut header))
        .map_err(install_error("读取更新包失败"))?;
    if header != *b"MZ" {
        return Err(UpdateError::Install(
            "Windows 更新包不是有效的 PE 可执行文件".into(),
        ));
    }
    Ok(())
}

fn sibling(target: &Path, suffix: &str) -> PathBuf {
    target.with_file_name(format!(".{}.{}", file_name(target), suffix))
}

/// 在进程退出后替换旧 exe，并启动新版本。脚本的路径作为进程参数传递，不参与
/// shell 拼接，安装目录中包含空格或引号时也不会改变命令含义。
pub(crate) fn relaunch_after_exit(target: &Path) -> Result<(), UpdateError> {
    let staged = sibling(target, STAGED_SUFFIX);
    if !staged.is_file() {
        return Err(UpdateError::Install("找不到已暂存的 Windows 更新包".into()));
    }
    let backup = sibling(target, BACKUP_SUFFIX);
    let script = sibling(target, SCRIPT_SUFFIX);
    remove_regular_file_if_exists(&script, "清理上次更新脚本失败")?;
    fs::write(&script, replacement_script()).map_err(install_error("写入更新脚本失败"))?;

    let powershell = powershell_path();
    Command::new(powershell)
        .args([
            OsStr::new("-NoProfile"),
            OsStr::new("-NonInteractive"),
            OsStr::new("-ExecutionPolicy"),
            OsStr::new("Bypass"),
            OsStr::new("-File"),
        ])
        .arg(&script)
        .arg(std::process::id().to_string())
        .arg(&staged)
        .arg(target)
        .arg(&backup)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(install_error("启动更新替换器失败"))
}

fn powershell_path() -> PathBuf {
    std::env::var_os("SystemRoot")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Windows"))
        .join(r"System32\WindowsPowerShell\v1.0\powershell.exe")
}

fn replacement_script() -> &'static str {
    r#"$ErrorActionPreference = 'Stop'
$processIdToWait = [int]$args[0]
$staged = $args[1]
$target = $args[2]
$backup = $args[3]

try { Wait-Process -Id $processIdToWait -ErrorAction SilentlyContinue } catch {}
Remove-Item -LiteralPath $backup -Force -ErrorAction SilentlyContinue
Move-Item -LiteralPath $target -Destination $backup -Force
try {
    Move-Item -LiteralPath $staged -Destination $target -Force
    Start-Process -FilePath $target
    Remove-Item -LiteralPath $backup -Force -ErrorAction SilentlyContinue
} catch {
    if (Test-Path -LiteralPath $backup) {
        Remove-Item -LiteralPath $target -Force -ErrorAction SilentlyContinue
        Move-Item -LiteralPath $backup -Destination $target -Force
    }
    throw
}
"#
}

pub(crate) fn cleanup_backups() {
    let Ok(target) = current_app() else {
        return;
    };
    cleanup_for(&target);
}

fn cleanup_for(target: &Path) {
    for (path, label) in [
        (sibling(target, STAGED_SUFFIX), "暂存更新包"),
        (sibling(target, BACKUP_SUFFIX), "旧版本备份"),
        (sibling(target, SCRIPT_SUFFIX), "更新脚本"),
    ] {
        if let Err(error) = remove_regular_file_if_exists(&path, "清理更新残留失败") {
            log::warn!(
                "清理 Windows {label} {} 失败：{}",
                path.display(),
                error.detail()
            );
        }
    }
}

fn remove_regular_file_if_exists(path: &Path, context: &str) -> Result<(), UpdateError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_file() || metadata.file_type().is_symlink() => {
            fs::remove_file(path).map_err(install_error(context))
        }
        Ok(_) => Err(UpdateError::Install(format!(
            "{context}：{} 不是文件",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(UpdateError::Install(format!("{context}：{error}"))),
    }
}

fn install_error(message: &str) -> impl Fn(std::io::Error) -> UpdateError + '_ {
    move |error| UpdateError::Install(format!("{message}：{error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn installed_app_detection_rejects_cargo_output() {
        assert!(
            current_app_from(Path::new(r"C:\Users\Alice\AppData\Local\Buddy\buddy.exe")).is_ok()
        );
        assert!(current_app_from(Path::new(r"C:\repo\target\release\buddy.exe")).is_err());
        assert!(current_app_from(Path::new(r"C:\Buddy\other.exe")).is_err());
    }

    #[test]
    fn stage_copies_only_a_valid_pe_executable() {
        let directory = tempdir().unwrap();
        let archive = directory.path().join("Buddy_0.2.0_x86_64.exe");
        fs::write(&archive, b"MZ signed update").unwrap();
        let target = directory.path().join("buddy.exe");

        validate_update_executable(&archive).unwrap();
        stage_update(&archive, &target).unwrap();
        assert_eq!(
            fs::read(sibling(&target, STAGED_SUFFIX)).unwrap(),
            b"MZ signed update"
        );

        let invalid = directory.path().join("update.zip");
        fs::write(&invalid, b"MZ").unwrap();
        assert!(validate_update_executable(&invalid).is_err());
        fs::write(&archive, b"not a pe").unwrap();
        assert!(validate_update_executable(&archive).is_err());
    }

    #[test]
    fn cleanup_removes_only_updater_files() {
        let directory = tempdir().unwrap();
        let target = directory.path().join("buddy.exe");
        let user_file = directory.path().join("keep.txt");
        fs::write(&user_file, b"keep").unwrap();
        for suffix in [STAGED_SUFFIX, BACKUP_SUFFIX, SCRIPT_SUFFIX] {
            fs::write(sibling(&target, suffix), b"leftover").unwrap();
        }

        cleanup_for(&target);
        for suffix in [STAGED_SUFFIX, BACKUP_SUFFIX, SCRIPT_SUFFIX] {
            assert!(!sibling(&target, suffix).exists());
        }
        assert_eq!(fs::read(user_file).unwrap(), b"keep");
    }

    #[test]
    fn replacement_script_waits_then_rolls_back_on_failure() {
        let script = replacement_script();
        assert!(script.contains("Wait-Process -Id $processIdToWait"));
        assert!(script.contains("Move-Item -LiteralPath $target -Destination $backup"));
        assert!(script.contains("Move-Item -LiteralPath $backup -Destination $target"));
        assert!(script.contains("Start-Process -FilePath $target"));
    }

    #[test]
    fn replacement_script_restores_the_old_exe_when_launch_fails() {
        let directory = tempdir().unwrap();
        let target = directory.path().join("buddy.exe");
        let staged = sibling(&target, STAGED_SUFFIX);
        let backup = sibling(&target, BACKUP_SUFFIX);
        let script = sibling(&target, SCRIPT_SUFFIX);
        fs::write(&target, b"old executable").unwrap();
        // 足以通过 Move-Item，但不是可启动的 PE；Start-Process 必然失败，覆盖回滚路径。
        fs::write(&staged, b"MZ not an executable").unwrap();
        fs::write(&script, replacement_script()).unwrap();

        let status = Command::new(powershell_path())
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(&script)
            // 不存在的 PID 会立即让 Wait-Process 返回。
            .arg("2147483647")
            .arg(&staged)
            .arg(&target)
            .arg(&backup)
            .status()
            .unwrap();

        assert!(!status.success());
        assert_eq!(fs::read(&target).unwrap(), b"old executable");
        assert!(!backup.exists());
    }
}
