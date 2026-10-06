# Publish signed, same-commit CI artifacts for both platforms without rebuilding.
param(
    [Parameter(Mandatory)][string]$Version,
    [Parameter(Mandatory)][string]$NotesFile,
    [string]$Dir = ".release/$Version",
    [string]$SourceCommit
)
$ErrorActionPreference = "Stop"
Set-Location "$PSScriptRoot/../.."
. "$PSScriptRoot/../windows-env.ps1"
function Run([string]$Command, [string[]]$Arguments) {
    $ErrorActionPreference = "Continue"
    & $Command @Arguments
    $ErrorActionPreference = "Stop"
    if ($LASTEXITCODE -ne 0) { throw "$Command failed: $LASTEXITCODE" }
}
function AssertNewVersion {
    $online = Invoke-RestMethod -Uri $channelUrl -Headers @{ "Cache-Control" = "no-cache" }
    if ([version]$Version -le [version]$online.version) { throw "Version must exceed published $($online.version)" }
}
function UploadVerified([string]$File, [string]$Key) {
    Run "ossutil" @("cp", $File, "oss://buddy-release/$Key", "--endpoint", "https://oss-cn-beijing.aliyuncs.com", "-f", "--cache-control", "no-cache")
    $response = Invoke-WebRequest -UseBasicParsing -Method Head -Uri "$publicBase/$Key" -Headers @{ "Cache-Control" = "no-cache" }
    if ($response.StatusCode -ne 200) { throw "Public HEAD status $($response.StatusCode): $Key" }
    $remoteSize = 0L
    if (-not [long]::TryParse([string]$response.Headers["Content-Length"], [ref]$remoteSize)) {
        throw "Public response has no valid Content-Length: $Key"
    }
    $localSize = (Get-Item -LiteralPath $File).Length
    if ($remoteSize -ne $localSize) { throw "Public Content-Length mismatch: $Key (remote=$remoteSize, local=$localSize)" }
}
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "Invalid version" }
if (-not $SourceCommit) { $SourceCommit = (& git rev-parse HEAD).Trim() }
if ($SourceCommit -notmatch '^[0-9a-f]{40}$' -or (& git rev-parse HEAD).Trim() -ne $SourceCommit) { throw "Current checkout must match the CI source commit" }
$tagCommit = (& git rev-parse "v${Version}^{}").Trim()
if ($LASTEXITCODE -ne 0 -or $tagCommit -ne $SourceCommit) { throw "Release tag and CI source commit must match" }
$remoteTag = & git ls-remote --exit-code --tags origin "refs/tags/v$Version" "refs/tags/v$Version^{}"
if ($LASTEXITCODE -ne 0 -or -not ($remoteTag -match "^$SourceCommit\s")) { throw "Source tag must be public before publishing" }
$publicBase = "https://buddy-release.oss-cn-beijing.aliyuncs.com"
$channelUrl = "$publicBase/buddy/channels/stable.json"
AssertNewVersion
$relativeAssets = @(
    "macos/aarch64/Buddy_${Version}_aarch64.app.tar.xz",
    "macos/aarch64/Buddy_${Version}_aarch64.dmg",
    "windows/x86_64/Buddy_${Version}_x86_64.exe"
)
$windowsSetup = "windows/x86_64/Buddy_${Version}_x86_64_setup.exe"
$windowsZip = "windows/x86_64/Buddy_${Version}_x86_64.zip"
if (Test-Path -LiteralPath (Join-Path $Dir $windowsSetup)) {
    $relativeAssets += $windowsSetup
    # The portable ZIP is optional for setup releases, but never uploaded unsigned.
    $windowsZipPath = Join-Path $Dir $windowsZip
    $windowsZipSignaturePath = "$windowsZipPath.sig"
    if ((Test-Path -LiteralPath $windowsZipPath) -and (Test-Path -LiteralPath $windowsZipSignaturePath) -and
        (Get-Item -LiteralPath $windowsZipPath).Length -gt 0 -and (Get-Item -LiteralPath $windowsZipSignaturePath).Length -gt 0) {
        $relativeAssets += $windowsZip
    }
} else {
    # Compatibility with releases whose CI artifacts predate setup installers.
    $relativeAssets += $windowsZip
}
# The client verifier rejects a different signing key, even if files have .sig names.
foreach ($relative in $relativeAssets) {
    $file = Join-Path $Dir $relative
    if (-not (Test-Path -LiteralPath $file) -or -not (Test-Path -LiteralPath "$file.sig")) { throw "Missing signed artifact: $relative" }
    Run "cargo" @("run", "--locked", "-p", "buddy-update", "--example", "verify-artifact", "--", $file, "$file.sig")
}
Run "node" @("scripts/release/manifest.mjs", "--version", $Version, "--notes-file", $NotesFile,
    "--base-url", "$publicBase/buddy/releases/$Version", "--source-url", "https://github.com/dcdyouget/buddy/tree/v$Version",
    "--dir", $Dir, "--output", "$Dir/manifest.json", "--platforms", "darwin-aarch64,windows-x86_64")
foreach ($relative in $relativeAssets) {
    foreach ($suffix in @("", ".sig")) { UploadVerified (Join-Path $Dir "$relative$suffix") "buddy/releases/$Version/$relative$suffix" }
}
UploadVerified "$Dir/manifest.json" "buddy/releases/$Version/manifest.json"
# Publish the download mirrors before switching the updater channel.
$releaseAssets = @("$Dir/macos/aarch64/Buddy_${Version}_aarch64.dmg")
if ($relativeAssets -contains $windowsSetup) {
    $releaseAssets += (Join-Path $Dir $windowsSetup)
}
$ErrorActionPreference = "Continue"
& gh release view "v$Version" --repo dcdyouget/buddy --json tagName 2>$null | Out-Null
$releaseExists = $LASTEXITCODE -eq 0
$ErrorActionPreference = "Stop"
if ($releaseExists) {
    # Reruns must also remove a ZIP attached by an older invocation; clobbering
    # the desired assets alone would leave the GitHub Release with stale extras.
    $releaseInfo = (& gh release view "v$Version" --repo "dcdyouget/buddy" --json assets 2>$null | ConvertFrom-Json)
    if ($LASTEXITCODE -eq 0) {
        $portableName = Split-Path -Leaf $windowsZip
        $stalePortable = @($releaseInfo.assets | Where-Object { $_.name -eq $portableName })
        if ($stalePortable.Count -gt 0) {
            Run "gh" @("release", "delete-asset", "v$Version", $portableName, "--repo", "dcdyouget/buddy", "--yes")
        }
    }
    Run "gh" (@("release", "upload", "v$Version") + $releaseAssets + @("--repo", "dcdyouget/buddy", "--clobber"))
} else {
    Run "gh" (@("release", "create", "v$Version") + $releaseAssets + @("--repo", "dcdyouget/buddy", "--verify-tag", "--latest",
        "--title", "Buddy $Version", "--notes-file", $NotesFile))
}
AssertNewVersion
UploadVerified "$Dir/manifest.json" "buddy/channels/stable.json"
Write-Host "Published Buddy ${Version}: $channelUrl"
