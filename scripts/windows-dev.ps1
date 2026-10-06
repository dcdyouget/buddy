# 自动加载已安装的 VS C++ / CMake / Ninja 工具；默认开发运行。
param([ValidateSet("run", "build", "check", "test")][string]$Action = "run")
$ErrorActionPreference = "Stop"
Set-Location "$PSScriptRoot/.."
. "$PSScriptRoot/windows-env.ps1"
# PowerShell 5 将重定向后的原生 stderr 包装成错误；Cargo 的进度也写入 stderr。
$ErrorActionPreference = "Continue"
switch ($Action) {
    "run" { & cargo run --locked -p buddy-app }
    "build" { & cargo build --release --locked -p buddy-app }
    "check" { & cargo check --workspace --all-targets --exclude buddy-markdown --locked }
    "test" { & cargo test --workspace --exclude buddy-markdown --locked }
}
exit $LASTEXITCODE
