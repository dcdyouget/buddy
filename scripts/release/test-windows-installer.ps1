# Installs and uninstalls an NSIS installer in an isolated directory; it never launches Buddy.
param(
    [Parameter(Mandatory)][string]$Setup,
    [Parameter(Mandatory)][string]$AppName,
    [Parameter(Mandatory)][string]$AppId,
    [string]$WorkDir = (Join-Path $env:RUNNER_TEMP ("buddy-installer-smoke-" + [guid]::NewGuid()))
)
$ErrorActionPreference = "Stop"
$setupPath = (Resolve-Path -LiteralPath $Setup).Path
$workPath = [IO.Path]::GetFullPath($WorkDir)
$installPath = Join-Path $workPath "installed"
New-Item -ItemType Directory -Force -Path $workPath | Out-Null

function Invoke-Installer([string]$FilePath, [string[]]$Arguments) {
    $process = Start-Process -FilePath $FilePath -ArgumentList $Arguments -Wait -PassThru -WindowStyle Hidden
    if ($process.ExitCode -ne 0) { throw "安装器退出码为 $($process.ExitCode)：$FilePath" }
}

Invoke-Installer $setupPath @("/S", "/D=$installPath")
$required = @("buddy.exe", "VCRUNTIME140.dll", "icon.ico", "Uninstall $AppName.exe")
$missing = $required | Where-Object { -not (Test-Path -LiteralPath (Join-Path $installPath $_)) }
if ($missing) { throw "安装后缺少文件：$($missing -join ', ')" }

$uninstallKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$AppId"
if (-not (Test-Path -LiteralPath $uninstallKey)) { throw "未写入卸载注册表：$uninstallKey" }
$startMenu = Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs\$AppName\$AppName.lnk"
if (-not (Test-Path -LiteralPath $startMenu)) { throw "未创建开始菜单快捷方式：$startMenu" }

# 同一路径覆盖安装也必须成功，且卸载项继续指向该安装目录。
Invoke-Installer $setupPath @("/S", "/D=$installPath")
if ((Get-ItemProperty -LiteralPath $uninstallKey).InstallLocation -ne $installPath) { throw "覆盖安装后的安装目录不一致" }

$uninstaller = Join-Path $installPath "Uninstall $AppName.exe"
Invoke-Installer $uninstaller @("/S")
if (Test-Path -LiteralPath $uninstallKey) { throw "卸载后注册表仍存在：$uninstallKey" }
if (Test-Path -LiteralPath $startMenu) { throw "卸载后开始菜单快捷方式仍存在：$startMenu" }
if (Test-Path -LiteralPath (Join-Path $installPath "buddy.exe")) { throw "卸载后 buddy.exe 仍存在：$installPath" }
Write-Host "Windows 安装器 smoke 通过：$setupPath"
