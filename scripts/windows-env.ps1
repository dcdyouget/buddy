# 仅设置当前进程的 C++ 构建环境；不修改系统 PATH 或用户配置。
$ErrorActionPreference = "Stop"

function Test-BuddyWindowsToolchain {
    foreach ($tool in @("cl", "cmake", "ninja", "cargo")) {
        if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
            return $false
        }
    }
    return $true
}

function Join-BuddyUniquePath {
    param([string[]]$Parts)

    $seen = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::OrdinalIgnoreCase)
    $items = foreach ($part in $Parts) {
        if ([string]::IsNullOrWhiteSpace($part)) { continue }
        foreach ($entry in $part -split ';') {
            $trimmed = $entry.Trim()
            if ($trimmed -and $seen.Add($trimmed)) {
                $trimmed
            }
        }
    }
    return $items -join ';'
}

# windows-dev.ps1 may dot-source this script several times in the same shell.
# VsDevCmd inherits PATH, so invoking it again after initialization can make the
# command line exceed cmd.exe's length limit.
if ($env:BUDDY_WINDOWS_BUILD_ENV_READY -eq "1" -and (Test-BuddyWindowsToolchain)) {
    return
}

$buddyOriginalPath = $env:PATH
$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio/Installer/vswhere.exe"
if (-not (Test-Path -LiteralPath $vswhere)) { throw "请安装 Visual Studio 2022 的 C++ 桌面开发组件" }
$visualStudio = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $visualStudio) { throw "找不到 Visual Studio C++ 工具链" }
$devCmd = Join-Path $visualStudio "Common7/Tools/VsDevCmd.bat"
$compilerEnv = & $env:ComSpec /d /s /c "`"$devCmd`" -no_logo -arch=x64 -host_arch=x64 && set"
if ($LASTEXITCODE -ne 0) { throw "加载 Visual Studio 开发环境失败" }

$vsPath = $null
foreach ($line in $compilerEnv) {
    if ($line -match '^(PATH|INCLUDE|LIB|LIBPATH)=(.*)$') {
        if ($matches[1] -eq "PATH") {
            $vsPath = $matches[2]
        }
        else {
            Set-Item -Path "Env:$($matches[1])" -Value $matches[2]
        }
    }
}
$vsPath = if ($null -ne $vsPath) { $vsPath } else { $env:PATH }
$cmakeBin = Join-Path $visualStudio "Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin"
$ninjaBin = Join-Path $visualStudio "Common7/IDE/CommonExtensions/Microsoft/CMake/Ninja"
$env:PATH = Join-BuddyUniquePath @($cmakeBin, $ninjaBin, $vsPath, $buddyOriginalPath)
foreach ($tool in @("cl", "cmake", "ninja", "cargo")) { Get-Command $tool -ErrorAction Stop | Out-Null }
$env:BUDDY_WINDOWS_BUILD_ENV_READY = "1"
