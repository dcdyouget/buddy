#!/usr/bin/env bash
# Buddy 一键发版（macOS Apple Silicon）。完整说明见 docs/release-workflow.md。
#
# 用法：
#   npm run release                                   # 交互输入版本号与更新说明
#   npm run release -- 0.2.0 --notes "修复 xxx"
#   npm run release -- 0.2.0 --notes-file notes.txt --yes
# 选项：
#   --notes <文本> / --notes-file <文件>   更新说明（必填，二选一；都不给则打开编辑器）
#   --skip-tests                           跳过测试（仅用于重跑失败的上传 / 发布）
#   --skip-publish                         只构建、上传版本目录，不改 channels/stable.json
#   --yes                                  发布前不再询问确认
#   --windows-dir <目录>                    使用本地未签名 Windows 制品，跳过 GitHub Actions 下载
#
# 签名私钥密码读取顺序：环境变量 TAURI_SIGNING_PRIVATE_KEY_PASSWORD →
#   钥匙串（security add-generic-password -s buddy-updater-key -a buddy -w）→ 终端输入。
# 签名私钥：~/.tauri/buddy-v2.key（公钥内置于 crates/update/src/lib.rs）。
#
# 「覆盖 channels/stable.json」让应用内更新看到新版本；之前任何一步失败都不影响用户。
# 之后推送 main 与版本标签到 GitHub，并创建附带 DMG 的 GitHub Release（源码即该标签）。
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$ROOT"

OSS_BUCKET="buddy-release"
OSS_ENDPOINT="https://oss-cn-beijing.aliyuncs.com"
PUBLIC_BASE="https://buddy-release.oss-cn-beijing.aliyuncs.com"
PREFIX="buddy"
# 与 crates/update/src/lib.rs 的 MANIFEST_URL 一致
CHANNEL_KEY="$PREFIX/channels/stable.json"
CHANNEL_URL="$PUBLIC_BASE/$CHANNEL_KEY"
KEY_PATH="${TAURI_SIGNING_PRIVATE_KEY_PATH:-$HOME/.tauri/buddy-v2.key}"
TAURI="$ROOT/node_modules/.bin/tauri"
# 制品也用 no-cache：浏览器每次向 OSS 校验 ETag（未变化时 304，不重复下载）。
ARTIFACT_CACHE="no-cache"
GITHUB_REPO="dcdyouget/buddy"

VERSION=""
NOTES=""
NOTES_FILE=""
SKIP_TESTS=false
SKIP_PUBLISH=false
YES=false
WINDOWS_DIR=""
HAS_WINDOWS=false
SOURCE_PUSHED=false
WINDOWS_WORKFLOW="Windows"
WINDOWS_ARTIFACT="Buddy-windows-x86_64"

fail() { printf '\n错误：%s\n' "$*" >&2; exit 1; }
step() { printf '\n==> [%s] %s\n' "$1" "$2"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --notes) NOTES="${2:?--notes 需要内容}"; shift ;;
    --notes-file) NOTES_FILE="${2:?--notes-file 需要路径}"; shift ;;
    --skip-tests) SKIP_TESTS=true ;;
    --skip-publish) SKIP_PUBLISH=true ;;
    --yes) YES=true ;;
    --windows-dir) WINDOWS_DIR="${2:?--windows-dir 需要 Windows 制品目录}"; shift ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    v[0-9]*|[0-9]*) VERSION="${1#v}" ;;
    *) fail "未知参数：$1" ;;
  esac
  shift
done

workspace_version() {
  awk '/^\[workspace\.package\]/{p=1;next} /^\[/{p=0} p&&/^version/{gsub(/.*= *"|".*/,"");print;exit}' Cargo.toml
}

remote_version() {
  # 404 视为尚未发布过
  local body
  body="$(curl -sS --fail -H 'Cache-Control: no-cache' "$CHANNEL_URL" 2>/dev/null)" || { echo ""; return; }
  node -e 'console.log(JSON.parse(process.argv[1]).version)' "$body"
}

version_gt() { # $1 > $2 ?
  node -e '
    const p = (v) => v.split(".").map(Number);
    const [a, b] = [p(process.argv[1]), p(process.argv[2])];
    for (let i = 0; i < 3; i++) if (a[i] !== b[i]) process.exit(a[i] > b[i] ? 0 : 1);
    process.exit(1);' "$1" "$2"
}

upload() { # 本地文件 OSS键 Cache-Control
  ossutil cp "$1" "oss://$OSS_BUCKET/$2" --endpoint "$OSS_ENDPOINT" -f --cache-control "$3" >/dev/null
}

verify_public() { # 公开URL 本地文件：公网下载回来逐字节比对 sha256
  local remote
  remote="$(curl -sS --fail --retry 3 -H 'Cache-Control: no-cache' "$1" | shasum -a 256 | cut -d' ' -f1)" \
    || fail "公网下载失败：$1"
  [[ "$remote" == "$(shasum -a 256 "$2" | cut -d' ' -f1)" ]] || fail "公网内容与本地不一致：$1"
}

copy_windows_artifacts() {
  local source="$1" file
  mkdir -p "$OUT/windows/x86_64"
  for file in "Buddy_${VERSION}_x86_64.exe" "Buddy_${VERSION}_x86_64_setup.exe" "Buddy_${VERSION}_x86_64.zip"; do
    [[ -f "$source/$file" ]] || fail "缺少同版本 Windows 制品：$source/$file"
    cp "$source/$file" "$OUT/windows/x86_64/$file"
  done
}

single_windows_artifact() {
  local root="$1" name="$2"
  local matches=()
  while IFS= read -r path; do matches+=("$path"); done < <(find "$root" -type f -name "$name" -print)
  [[ ${#matches[@]} -eq 1 ]] || fail "GitHub Actions 制品中应恰好有一个 $name，实际为 ${#matches[@]} 个"
  printf '%s\n' "${matches[0]}"
}

download_windows_artifacts() {
  local head="$1" started_at runs run_id download_dir exe setup zip
  started_at="$(date -u +%s)"
  gh workflow run "$WINDOWS_WORKFLOW" --repo "$GITHUB_REPO" --ref main -f ref="$head" \
    || fail "无法触发 Windows 构建工作流"
  echo "已触发 Windows 构建，等待提交 ${head:0:12} 的工作流..."

  run_id=""
  for _ in {1..60}; do
    runs="$(gh run list --repo "$GITHUB_REPO" --workflow "$WINDOWS_WORKFLOW" --event workflow_dispatch \
      --commit "$head" --limit 20 --json databaseId,headSha,createdAt 2>/dev/null)" || runs="[]"
    run_id="$(node -e '
      const [head, started, runs] = process.argv.slice(1);
      const candidates = JSON.parse(runs).filter(run =>
        run.headSha === head && Date.parse(run.createdAt) >= (Number(started) - 5) * 1000
      ).sort((a, b) => Date.parse(b.createdAt) - Date.parse(a.createdAt));
      if (candidates[0]) process.stdout.write(String(candidates[0].databaseId));
    ' "$head" "$started_at" "$runs")"
    [[ -n "$run_id" ]] && break
    sleep 2
  done
  [[ -n "$run_id" ]] || fail "未找到刚触发且 HEAD 为 $head 的 Windows 工作流"

  gh run watch "$run_id" --repo "$GITHUB_REPO" --exit-status \
    || fail "Windows 构建失败：gh run view $run_id --repo $GITHUB_REPO --log-failed"
  [[ "$(gh run view "$run_id" --repo "$GITHUB_REPO" --json headSha --jq .headSha)" == "$head" ]] \
    || fail "Windows 工作流 $run_id 的 HEAD 与发布提交不一致"

  download_dir="$OUT/windows-ci"
  rm -rf "$download_dir"
  mkdir -p "$download_dir"
  gh run download "$run_id" --repo "$GITHUB_REPO" --name "$WINDOWS_ARTIFACT" --dir "$download_dir" \
    || fail "无法下载 Windows 构建制品"
  exe="$(single_windows_artifact "$download_dir" "Buddy_${VERSION}_x86_64.exe")"
  setup="$(single_windows_artifact "$download_dir" "Buddy_${VERSION}_x86_64_setup.exe")"
  zip="$(single_windows_artifact "$download_dir" "Buddy_${VERSION}_x86_64.zip")"
  mkdir -p "$OUT/windows/x86_64"
  cp "$exe" "$OUT/windows/x86_64/$(basename "$exe")"
  cp "$setup" "$OUT/windows/x86_64/$(basename "$setup")"
  cp "$zip" "$OUT/windows/x86_64/$(basename "$zip")"
}

# ── 1. 环境与输入 ─────────────────────────────────────────────
step 1/11 "环境与仓库检查"
[[ "$(uname -s)/$(uname -m)" == "Darwin/arm64" ]] || fail "只能在 Apple Silicon Mac 上发布"
for c in git gh node cargo ossutil curl codesign hdiutil plutil shasum security; do
  command -v "$c" >/dev/null || fail "缺少命令：$c"
done
[[ -x "$TAURI" ]] || fail "缺少签名工具 $TAURI（先执行 npm install）"
[[ -f "$KEY_PATH" ]] || fail "缺少签名私钥：$KEY_PATH"
[[ "$(git branch --show-current)" == "main" ]] || fail "请在 main 分支发布"
[[ -z "$(git status --porcelain)" ]] || fail "工作区有未提交改动，请先提交或清理"
gh auth status >/dev/null 2>&1 || fail "gh 未登录 GitHub（gh auth login）"
# GitHub 上的 main 必须是本地 main 的祖先，否则发布后推送会失败
git fetch -q origin main || fail "无法从 GitHub 拉取 main"
git merge-base --is-ancestor origin/main HEAD || fail "GitHub 上的 main 有本地没有的提交，请先同步"

if [[ -z "$VERSION" ]]; then
  [[ -t 0 ]] || fail "非交互模式必须给出版本号"
  read -r -p "当前版本 $(workspace_version)，请输入新版本号：" VERSION
fi
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "版本号必须是 主.次.修订：$VERSION"
PUBLISHED="$(remote_version)"
git rev-parse -q --verify "refs/tags/v$VERSION" >/dev/null && fail "标签 v$VERSION 已存在"
if [[ -n "$PUBLISHED" ]]; then
  version_gt "$VERSION" "$PUBLISHED" || fail "新版本 $VERSION 必须高于线上版本 $PUBLISHED"
fi
echo "线上版本：${PUBLISHED:-（尚未发布）} → 新版本：$VERSION"

OUT="$ROOT/.release/$VERSION"
mkdir -p "$OUT"
if [[ -n "$WINDOWS_DIR" ]]; then
  copy_windows_artifacts "$WINDOWS_DIR"
  HAS_WINDOWS=true
fi
if [[ -n "$NOTES_FILE" ]]; then
  cp "$NOTES_FILE" "$OUT/notes.txt"
elif [[ -n "$NOTES" ]]; then
  printf '%s\n' "$NOTES" >"$OUT/notes.txt"
else
  [[ -t 0 ]] || fail "非交互模式必须给出 --notes 或 --notes-file"
  printf '# 在下方填写 %s 的更新说明（中文），以 # 开头的行会被忽略\n' "$VERSION" >"$OUT/notes.txt"
  "${EDITOR:-vi}" "$OUT/notes.txt"
  grep -v '^#' "$OUT/notes.txt" >"$OUT/notes.tmp" || true
  mv "$OUT/notes.tmp" "$OUT/notes.txt"
fi
[[ -n "$(tr -d '[:space:]' <"$OUT/notes.txt")" ]] || fail "更新说明不能为空"

if [[ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD+x}" ]]; then
  if TAURI_SIGNING_PRIVATE_KEY_PASSWORD="$(security find-generic-password -s buddy-updater-key -a buddy -w 2>/dev/null)"; then
    echo "签名私钥密码：读取自钥匙串"
  else
    [[ -t 0 ]] || fail "非交互模式需要钥匙串中的 buddy-updater-key 或环境变量 TAURI_SIGNING_PRIVATE_KEY_PASSWORD"
    read -r -s -p "请输入签名私钥密码：" TAURI_SIGNING_PRIVATE_KEY_PASSWORD; echo
  fi
fi
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD
sign() { "$TAURI" signer sign -f "$KEY_PATH" "$1" </dev/null >/dev/null 2>&1 || fail "签名失败：$1（检查私钥密码）"; }
# 先试签一个临时文件：密码错误时在编译前就失败
PROBE="$(mktemp)"; sign "$PROBE"; rm -f "$PROBE" "$PROBE.sig"

# ── 2. 版本号 ─────────────────────────────────────────────────
step 2/11 "写入版本号并提交"
if [[ "$(workspace_version)" != "$VERSION" ]]; then
  node scripts/set-version.mjs "$VERSION"
  cargo metadata --format-version 1 >/dev/null # 同步 Cargo.lock 中的 workspace 版本
  git add Cargo.toml Cargo.lock
  git commit -q -m "chore(release): v$VERSION"
  echo "已提交 chore(release): v$VERSION"
else
  echo "版本号已是 $VERSION，跳过"
fi
COMMIT="$(git rev-parse --short HEAD)"
COMMIT_FULL="$(git rev-parse HEAD)"

# ── 3. 测试 ───────────────────────────────────────────────────
step 3/11 "提交门禁与测试"
if [[ "$SKIP_TESTS" == true ]]; then
  echo "已跳过（--skip-tests）"
else
  scripts/gate.sh >"$OUT/gate.log" 2>&1 || { tail -30 "$OUT/gate.log"; fail "提交门禁未通过（日志：$OUT/gate.log）"; }
  cargo test --workspace --exclude buddy-markdown >"$OUT/test.log" 2>&1 \
    || { grep -E "FAILED|panicked|error" "$OUT/test.log" | head -20; fail "测试失败（日志：$OUT/test.log）"; }
  echo "门禁与测试通过"
fi

# GitHub Actions 必须从已公开的精确提交检出；不能把当前工作目录或后续 main 的
# 构建产物误当作本次发布制品。显式传入 commit SHA 后，工作流 checkout 该 SHA。
if [[ "$SKIP_PUBLISH" != true && -z "$WINDOWS_DIR" ]]; then
  step 4/11 "推送发布源码并构建 Windows 制品"
  git push -q origin HEAD:main \
    || fail "无法推送发布提交，Windows 构建未触发"
  SOURCE_PUSHED=true
  download_windows_artifacts "$COMMIT_FULL"
  HAS_WINDOWS=true
fi

# ── 5-6. 构建、组装、签名 ─────────────────────────────────────
step 5/11 "构建 release 二进制"
MACOSX_DEPLOYMENT_TARGET=12.0 cargo build --release --locked -p buddy-app --bin buddy >"$OUT/build.log" 2>&1 \
  || { tail -30 "$OUT/build.log"; fail "构建失败（日志：$OUT/build.log）"; }

step 6/11 "组装 Buddy.app / 更新包 / 安装包并签名"
MAC_DIR="$OUT/macos/aarch64"
mkdir -p "$MAC_DIR"
scripts/release/bundle-macos.sh "$VERSION" "$ROOT/target/release/buddy" "$MAC_DIR"
[[ "$(plutil -extract CFBundleShortVersionString raw -o - "$MAC_DIR/Buddy.app/Contents/Info.plist")" == "$VERSION" ]] \
  || fail "Info.plist 版本与 $VERSION 不一致"
UPDATE="$MAC_DIR/Buddy_${VERSION}_aarch64.app.tar.xz"
INSTALLER="$MAC_DIR/Buddy_${VERSION}_aarch64.dmg"
sign "$UPDATE"
sign "$INSTALLER"
if [[ "$HAS_WINDOWS" == true ]]; then
  WINDOWS_UPDATE="$OUT/windows/x86_64/Buddy_${VERSION}_x86_64.exe"
  WINDOWS_INSTALLER="$OUT/windows/x86_64/Buddy_${VERSION}_x86_64_setup.exe"
  WINDOWS_PORTABLE="$OUT/windows/x86_64/Buddy_${VERSION}_x86_64.zip"
  sign "$WINDOWS_UPDATE"
  sign "$WINDOWS_INSTALLER"
  sign "$WINDOWS_PORTABLE"
  cargo run --locked -p buddy-update --example verify-artifact -- "$WINDOWS_UPDATE" "$WINDOWS_UPDATE.sig" \
    || fail "Windows EXE 更新签名验证失败"
  cargo run --locked -p buddy-update --example verify-artifact -- "$WINDOWS_INSTALLER" "$WINDOWS_INSTALLER.sig" \
    || fail "Windows 安装器签名验证失败"
  cargo run --locked -p buddy-update --example verify-artifact -- "$WINDOWS_PORTABLE" "$WINDOWS_PORTABLE.sig" \
    || fail "Windows 便携包签名验证失败"
fi
REMOTE_DIR="$PREFIX/releases/$VERSION"
MANIFEST_PLATFORMS="darwin-aarch64"
if [[ "$HAS_WINDOWS" == true ]]; then MANIFEST_PLATFORMS="darwin-aarch64,windows-x86_64"; fi
node scripts/release/manifest.mjs --version "$VERSION" --notes-file "$OUT/notes.txt" \
  --base-url "$PUBLIC_BASE/$REMOTE_DIR" --source-url "https://github.com/$GITHUB_REPO/tree/v$VERSION" \
  --dir "$OUT" --output "$OUT/manifest.json" --platforms "$MANIFEST_PLATFORMS"

# ── 7. 上传版本目录 ───────────────────────────────────────────
step 7/11 "上传到 OSS 并公网回读校验"
for f in "$UPDATE" "$UPDATE.sig" "$INSTALLER" "$INSTALLER.sig"; do
  upload "$f" "$REMOTE_DIR/macos/aarch64/$(basename "$f")" "$ARTIFACT_CACHE"
done
if [[ "$HAS_WINDOWS" == true ]]; then
  for f in "$OUT/windows/x86_64/"*; do
    upload "$f" "$REMOTE_DIR/windows/x86_64/$(basename "$f")" "$ARTIFACT_CACHE"
    verify_public "$PUBLIC_BASE/$REMOTE_DIR/windows/x86_64/$(basename "$f")" "$f"
  done
fi
upload "$OUT/manifest.json" "$REMOTE_DIR/manifest.json" "no-cache"
for f in "$UPDATE" "$INSTALLER"; do
  verify_public "$PUBLIC_BASE/$REMOTE_DIR/macos/aarch64/$(basename "$f")" "$f"
done
verify_public "$PUBLIC_BASE/$REMOTE_DIR/manifest.json" "$OUT/manifest.json"
echo "版本目录已上传：$PUBLIC_BASE/$REMOTE_DIR/"

if [[ "$SKIP_PUBLISH" == true ]]; then
  echo; echo "已跳过发布（--skip-publish）。用户不会看到 $VERSION。"
  exit 0
fi

# ── 8. 确认并发布 ─────────────────────────────────────────────
step 8/11 "发布确认"
if [[ "$YES" != true ]]; then
  [[ -t 0 ]] || fail "非交互模式发布需要 --yes"
  read -r -p "即将正式发布 $VERSION，用户将能看到此更新。确认？(y/N) " answer
  [[ "$answer" == "y" || "$answer" == "Y" ]] || fail "已取消；版本目录已上传但未发布"
fi

step 9/11 "打标签并推送到 GitHub，确保源码先公开"
git tag -a "v$VERSION" -m "Buddy v$VERSION" "$COMMIT"
if [[ "$SOURCE_PUSHED" == true ]]; then
  git push -q origin "v$VERSION" \
    || fail "标签推送失败，stable.json 尚未切换。请先修复源码推送，再发布已暂存的清单"
else
  git push -q origin main "v$VERSION" \
    || fail "推送失败，stable.json 尚未切换。请先修复源码推送，再发布已暂存的清单"
fi

step 10/11 "更新固定地址 channels/stable.json"
upload "$OUT/manifest.json" "$CHANNEL_KEY" "no-cache"
verify_public "$CHANNEL_URL" "$OUT/manifest.json"

step 11/11 "创建 GitHub Release"
{ cat "$OUT/notes.txt"; printf '\n安装：下载 `%s`，打开后把 Buddy 拖进「应用程序」。仅支持 Apple Silicon，macOS 12 及以上；首次打开若被系统拦截，请右键「打开」。\n' "$(basename "$INSTALLER")"; } >"$OUT/github-notes.md"
if [[ "$HAS_WINDOWS" == true ]]; then
  printf '\nWindows 10/11 x64：下载 `Buddy_%s_x86_64_setup.exe`，双击按向导安装。默认快捷键 Ctrl+J；ZIP 为可选便携版。\n' "$VERSION" >>"$OUT/github-notes.md"
fi
RELEASE_ASSETS=("$INSTALLER")
# GitHub Release 只提供用户直接安装的包；portable ZIP 仍已上传到 OSS，
# 并由 manifest 作为可选下载项保留。
if [[ "$HAS_WINDOWS" == true ]]; then RELEASE_ASSETS+=("$WINDOWS_INSTALLER"); fi
gh release create "v$VERSION" "${RELEASE_ASSETS[@]}" --repo "$GITHUB_REPO" --verify-tag --latest \
  --title "Buddy $VERSION" --notes-file "$OUT/github-notes.md" >/dev/null \
  || fail "创建 GitHub Release 失败（OSS 已发布、标签已推送）。手动执行：gh release create v$VERSION $INSTALLER --title \"Buddy $VERSION\" --notes-file $OUT/github-notes.md"

INSTALLER_URL="$(node -e 'console.log(require(process.argv[1]).platforms["darwin-aarch64"].installer.url)' "$OUT/manifest.json")"
printf '\n发布完成：Buddy %s（commit %s，标签 v%s 已推送）\n' "$VERSION" "$COMMIT" "$VERSION"
printf '  版本信息（固定地址）：%s\n' "$CHANNEL_URL"
printf '  安装包下载（OSS）：%s\n' "$INSTALLER_URL"
printf '  GitHub Release：https://github.com/%s/releases/tag/v%s\n' "$GITHUB_REPO" "$VERSION"
