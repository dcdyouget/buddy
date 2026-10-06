# Windows 便携安装包及原始 EXE 更新包。签名由 release-windows.ps1 完成。
param(
    [Parameter(Mandatory)][string]$Version,
    [string]$Binary = "target/release/buddy.exe",
    [string]$OutDir = ".release/$Version/windows/x86_64",
    [string]$InstallerIcon
)
$ErrorActionPreference = "Stop"
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "版本号必须是 主.次.修订" }
function Find-VcRuntime140 {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path -LiteralPath $vswhere)) { throw "缺少 vswhere.exe，无法定位 Visual C++ 可再发行运行库" }

    $installations = & $vswhere -products * -requires Microsoft.VisualStudio.Component.VC.Redist.14.Latest -property installationPath
    if ($LASTEXITCODE -ne 0) { throw "vswhere 无法定位已安装的 Visual C++ Redistributable 组件" }
    $candidates = foreach ($installation in $installations) {
        $redistRoot = Join-Path $installation "VC\Redist\MSVC"
        if (-not (Test-Path -LiteralPath $redistRoot)) { continue }
        foreach ($versionDir in Get-ChildItem -LiteralPath $redistRoot -Directory) {
            try { $version = [version]$versionDir.Name } catch { continue }
            $runtime = Join-Path $versionDir.FullName "x64\Microsoft.VC143.CRT\VCRUNTIME140.dll"
            if (Test-Path -LiteralPath $runtime) {
                [PSCustomObject]@{ Version = $version; Path = (Resolve-Path -LiteralPath $runtime).Path }
            }
        }
    }
    $selected = $candidates | Sort-Object Version -Descending | Select-Object -First 1
    if ($null -eq $selected) {
        throw "未找到 VC\Redist\MSVC\*\x64\Microsoft.VC143.CRT\VCRUNTIME140.dll；请安装 Visual Studio 的最新 Visual C++ Redistributable 组件"
    }
    Write-Host "使用 Microsoft VC++ Runtime $($selected.Version)：$($selected.Path)"
    return $selected.Path
}
function Find-Makensis {
    $candidates = @()
    $onPath = Get-Command "makensis.exe" -ErrorAction SilentlyContinue
    if ($null -ne $onPath) { $candidates += $onPath.Source }
    $candidates += @(
        (Join-Path ${env:ProgramFiles(x86)} "NSIS\makensis.exe"),
        (Join-Path $env:ProgramFiles "NSIS\makensis.exe")
    )
    $selected = $candidates | Where-Object { $_ -and (Test-Path -LiteralPath $_) } | Select-Object -First 1
    if ($null -eq $selected) {
        throw "未找到 NSIS 3 的 makensis.exe。请安装 NSIS 3，或将 makensis.exe 所在目录加入 PATH。"
    }
    return (Resolve-Path -LiteralPath $selected).Path
}
$repoRoot = (Resolve-Path "$PSScriptRoot/../..").Path
$binaryPath = (Resolve-Path -LiteralPath $Binary).Path
$vcRuntimePath = Find-VcRuntime140
$defaultInstallerIcon = Join-Path $repoRoot "apps\buddy\bundle\windows\icon.ico"
if (-not $InstallerIcon -and (Test-Path -LiteralPath $defaultInstallerIcon)) { $InstallerIcon = $defaultInstallerIcon }
New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$outputPath = (Resolve-Path -LiteralPath $OutDir).Path
$base = Join-Path $outputPath "Buddy_${Version}_x86_64"
$stageRoot = Join-Path $outputPath (".bundle-" + [guid]::NewGuid())
$stage = Join-Path $stageRoot "Buddy"
try {
    New-Item -ItemType Directory -Force -Path $stage | Out-Null
    Copy-Item -LiteralPath $binaryPath -Destination "$base.exe" -Force
    Copy-Item -LiteralPath $binaryPath -Destination (Join-Path $stage "buddy.exe") -Force
    Copy-Item -LiteralPath $vcRuntimePath -Destination (Join-Path $stage "VCRUNTIME140.dll") -Force
    if ($InstallerIcon) {
        $iconPath = (Resolve-Path -LiteralPath $InstallerIcon).Path
        if ([IO.Path]::GetExtension($iconPath) -ine ".ico") { throw "安装器图标必须是 .ico 文件：$iconPath" }
        Copy-Item -LiteralPath $iconPath -Destination (Join-Path $stage "icon.ico") -Force
    }
    else {
        throw "缺少 Windows 安装器图标：$defaultInstallerIcon"
    }
    foreach ($name in @("LICENSE", "LICENSE-GPL-3.0-or-later", "LICENSE-APACHE-2.0", "THIRD_PARTY_NOTICES.md")) {
        Copy-Item -LiteralPath (Join-Path $repoRoot $name) -Destination $stage -Force
    }
    Compress-Archive -LiteralPath $stage -DestinationPath "$base.zip" -Force

    $makensis = Find-Makensis
    $installerScript = Join-Path $PSScriptRoot "windows-installer.nsi"
    $installerPath = "$base`_setup.exe"
    $defines = @(
        "/DVERSION=$Version",
        "/DSTAGE_DIR=$stage",
        "/DOUTFILE=$installerPath"
    )
    $defines += "/DICON_FILE=$iconPath"
    & $makensis "/INPUTCHARSET" "UTF8" @defines $installerScript
    if ($LASTEXITCODE -ne 0) { throw "NSIS 编译安装器失败（退出码 $LASTEXITCODE）" }
    if (-not (Test-Path -LiteralPath $installerPath)) { throw "NSIS 未生成安装器：$installerPath" }
    Write-Host "已生成：$base.exe / $base.zip / $installerPath"
}
finally {
    if (Test-Path -LiteralPath $stageRoot) {
        $resolvedStage = (Resolve-Path -LiteralPath $stageRoot).Path
        if (-not $resolvedStage.StartsWith($outputPath + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
            throw "暂存目录超出了输出目录"
        }
        Remove-Item -LiteralPath $resolvedStage -Recurse -Force
    }
}
