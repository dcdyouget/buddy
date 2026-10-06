# 仅设置当前进程的 C++ 构建环境；不修改系统 PATH 或用户配置。
$ErrorActionPreference = "Stop"
$buddyOriginalPath = $env:PATH
$vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio/Installer/vswhere.exe"
if (-not (Test-Path -LiteralPath $vswhere)) { throw "请安装 Visual Studio 2022 的 C++ 桌面开发组件" }
$visualStudio = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $visualStudio) { throw "找不到 Visual Studio C++ 工具链" }
$devCmd = Join-Path $visualStudio "Common7/Tools/VsDevCmd.bat"
$compilerEnv = & $env:ComSpec /d /s /c "`"$devCmd`" -no_logo -arch=x64 -host_arch=x64 && set"
if ($LASTEXITCODE -ne 0) { throw "加载 Visual Studio 开发环境失败" }
foreach ($line in $compilerEnv) {
    if ($line -match '^(PATH|INCLUDE|LIB|LIBPATH)=(.*)$') {
        [Environment]::SetEnvironmentVariable($matches[1], $matches[2], "Process")
    }
}
$cmakeBin = Join-Path $visualStudio "Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin"
$ninjaBin = Join-Path $visualStudio "Common7/IDE/CommonExtensions/Microsoft/CMake/Ninja"
$env:PATH = "$cmakeBin;$ninjaBin;$env:PATH;$buddyOriginalPath"
foreach ($tool in @("cl", "cmake", "ninja", "cargo")) { Get-Command $tool -ErrorAction Stop | Out-Null }
