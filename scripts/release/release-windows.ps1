# 构建、签名、上传 Windows 包；与相同版本的 macOS 清单合并后才发布双平台更新。
param(
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$NotesFile,
    [string]$MergeManifest,
    [switch]$SkipTests,
    [switch]$SkipPublish
)
$ErrorActionPreference = "Stop"
Set-Location "$PSScriptRoot/../.."
. "$PSScriptRoot/../windows-env.ps1"
function Run([string]$Command, [string[]]$Arguments) {
    # 原生命令的成败由退出码判断，避免 PS5 把 Cargo 进度当作终止错误。
    $ErrorActionPreference = "Continue"
    & $Command @Arguments
    $ErrorActionPreference = "Stop"
    if ($LASTEXITCODE -ne 0) { throw "$Command 执行失败：$LASTEXITCODE" }
}
function Upload([string]$File, [string]$Key) {
    Run "ossutil" @("cp", $File, "oss://buddy-release/$Key", "--endpoint", "https://oss-cn-beijing.aliyuncs.com", "-f", "--cache-control", "no-cache")
}
function VerifyPublic([string]$File, [string]$Url) {
    $checkPath = "$File.public-check"
    try {
        Invoke-WebRequest -UseBasicParsing -Uri $Url -Headers @{ "Cache-Control" = "no-cache" } -OutFile $checkPath
        if ((Get-FileHash -LiteralPath $File).Hash -ne (Get-FileHash -LiteralPath $checkPath).Hash) { throw "公网校验失败：$Url" }
    } finally {
        if (Test-Path -LiteralPath $checkPath) { Remove-Item -LiteralPath $checkPath }
    }
}
function ReadChannel([string]$Url) {
    try { return Invoke-RestMethod -Uri $Url -Headers @{ "Cache-Control" = "no-cache" } }
    catch {
        if ($_.Exception.Response -and [int]$_.Exception.Response.StatusCode -eq 404) { return $null }
        throw
    }
}
if ($env:OS -ne "Windows_NT" -or $env:PROCESSOR_ARCHITECTURE -ne "AMD64") { throw "需要 Windows x64 构建环境" }
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "版本号必须是 主.次.修订" }
foreach ($command in @("cargo", "node", "ossutil")) { Get-Command $command -ErrorAction Stop | Out-Null }
Run "git" @("diff", "--exit-code", "HEAD", "--", ".")
if (& git status --porcelain) { throw "工作区必须先提交或清理后再发布" }
$stableUrl = "https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/channels/stable.json"
$online = ReadChannel $stableUrl
if ($online -and [version]$Version -le [version]$online.version) { throw "新版本必须高于线上版本 $($online.version)" }
$keyPath = $env:TAURI_SIGNING_PRIVATE_KEY_PATH
if (-not $keyPath) { $keyPath = Join-Path $env:USERPROFILE ".tauri/buddy-v2.key" }
if (-not (Test-Path -LiteralPath $keyPath)) { throw "缺少更新签名私钥：$keyPath" }
if ($null -eq $env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD) { throw "请先设置 TAURI_SIGNING_PRIVATE_KEY_PASSWORD（可为空）" }
$signer = (Resolve-Path "node_modules/.bin/tauri.cmd").Path
$metadata = (& cargo metadata --no-deps --format-version 1 | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0) { throw "无法读取 Cargo 版本" }
if (($metadata.packages | Where-Object name -eq "buddy-app").version -ne $Version) { throw "请先通过 npm run version:set -- $Version 同步版本号" }
$outDir = ".release/$Version"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$notes = Get-Content -Raw -Encoding UTF8 -LiteralPath $NotesFile
if (-not $notes.Trim()) { throw "更新说明不能为空" }
[IO.File]::WriteAllText((Join-Path (Resolve-Path $outDir).Path "notes.txt"), $notes, [Text.UTF8Encoding]::new($false))
$baseUrl = "https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/releases/$Version"
# 没有指定本地 Mac 清单时，从已暂存的同版本目录读取，绝不复用旧版本包。
if (-not $SkipPublish -and -not $MergeManifest) {
    $MergeManifest = "$outDir/merge-manifest.json"
    Invoke-WebRequest -UseBasicParsing -Uri "$baseUrl/manifest.json" -Headers @{ "Cache-Control" = "no-cache" } -OutFile $MergeManifest
}
if ($MergeManifest) {
    $merge = Get-Content -Raw -Encoding UTF8 -LiteralPath $MergeManifest | ConvertFrom-Json
    if ($merge.schema -ne 1 -or $merge.version -ne $Version) { throw "合并清单的版本必须为 $Version" }
}
if (-not $SkipTests) {
    Run "cargo" @("check", "--workspace", "--all-targets", "--exclude", "buddy-markdown", "--locked")
    Run "cargo" @("test", "--workspace", "--exclude", "buddy-markdown", "--locked")
    Run "node" @("--test", "scripts/release/manifest.test.mjs")
}
Run "cargo" @("build", "--release", "--locked", "-p", "buddy-app", "--bin", "buddy")
& "$PSScriptRoot/bundle-windows.ps1" -Version $Version
$windowsDir = "$outDir/windows/x86_64"
foreach ($suffix in @(".exe", "_setup.exe", ".zip")) {
    $asset = "$windowsDir/Buddy_${Version}_x86_64$suffix"
    Run $signer @("signer", "sign", "-f", $keyPath, $asset)
    Run "cargo" @("run", "--locked", "-p", "buddy-update", "--example", "verify-artifact", "--", $asset, "$asset.sig")
}
$manifestArgs = @("scripts/release/manifest.mjs", "--version", $Version, "--notes-file", "$outDir/notes.txt", "--base-url", $baseUrl,
    "--source-url", "https://github.com/dcdyouget/buddy/tree/v$Version", "--dir", $outDir, "--output", "$outDir/manifest.json", "--platforms", "windows-x86_64")
if ($MergeManifest) { $manifestArgs += @("--merge", $MergeManifest) }
Run "node" $manifestArgs
$manifest = Get-Content -Raw -Encoding UTF8 -LiteralPath "$outDir/manifest.json" | ConvertFrom-Json
if (-not $SkipPublish -and -not $manifest.platforms.'darwin-aarch64') { throw "正式发布必须包含同版本的 macOS 包" }
foreach ($suffix in @(".exe", ".exe.sig", "_setup.exe", "_setup.exe.sig", ".zip", ".zip.sig")) {
    $file = "$windowsDir/Buddy_${Version}_x86_64$suffix"
    Upload $file "buddy/releases/$Version/windows/x86_64/$(Split-Path -Leaf $file)"
    VerifyPublic $file "$baseUrl/windows/x86_64/$(Split-Path -Leaf $file)"
}
# 发布前逐一校验清单中所有平台的公开文件，避免发布不存在的 Mac 包。
foreach ($platform in $manifest.platforms.PSObject.Properties) {
    foreach ($asset in @($platform.Value.update, $platform.Value.installer)) {
        $checkFile = "$outDir/asset-check"
        try {
            Invoke-WebRequest -UseBasicParsing -Uri $asset.url -OutFile $checkFile
            if ((Get-Item -LiteralPath $checkFile).Length -ne $asset.size -or (Get-FileHash -LiteralPath $checkFile).Hash.ToLowerInvariant() -ne $asset.sha256) { throw "清单制品校验失败：$($asset.url)" }
        } finally { if (Test-Path -LiteralPath $checkFile) { Remove-Item -LiteralPath $checkFile } }
    }
}
if (-not $SkipPublish) {
    # 构建耗时较长，正式切换前再读一次，防止其他机器已发布更高版本。
    $online = ReadChannel $stableUrl
    if ($online -and [version]$Version -le [version]$online.version) { throw "新版本必须高于线上版本 $($online.version)" }
    # 源码标签必须先公开，保证 GPL 源码链接存在。脚本不自动提交或推送源码。
    $remoteTag = & git ls-remote --exit-code --tags origin "refs/tags/v$Version" "refs/tags/v$Version^{}"
    if ($LASTEXITCODE -ne 0) { throw "必须先公开源码标签 v$Version" }
    $tagLine = @($remoteTag | Where-Object { $_ -match '\^\{\}$' })
    if (-not $tagLine.Count) { $tagLine = @($remoteTag | Select-Object -First 1) }
    $headCommit = & git rev-parse HEAD
    if (($tagLine[0] -split '\s+')[0] -ne $headCommit) { throw "源码标签 v$Version 必须对应当前构建提交" }
}
Upload "$outDir/manifest.json" "buddy/releases/$Version/manifest.json"
VerifyPublic "$outDir/manifest.json" "$baseUrl/manifest.json"
if (-not $SkipPublish) {
    Upload "$outDir/manifest.json" "buddy/channels/stable.json"
    VerifyPublic "$outDir/manifest.json" $stableUrl
}
Write-Host "Windows 制品已上传。SkipPublish=$SkipPublish；版本 $Version"
