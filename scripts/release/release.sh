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
#
# 签名私钥密码读取顺序：环境变量 TAURI_SIGNING_PRIVATE_KEY_PASSWORD →
#   钥匙串（security add-generic-password -s buddy-updater-key -a buddy -w）→ 终端输入。
# 签名私钥：~/.tauri/buddy-v2.key（公钥内置于 crates/update/src/lib.rs）。
#
# 只有最后一步「覆盖 channels/stable.json」会让用户看到新版本；之前任何一步失败都不影响用户。
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
IMMUTABLE="public,max-age=31536000,immutable"

VERSION=""
NOTES=""
NOTES_FILE=""
SKIP_TESTS=false
SKIP_PUBLISH=false
YES=false

fail() { printf '\n错误：%s\n' "$*" >&2; exit 1; }
step() { printf '\n==> [%s] %s\n' "$1" "$2"; }

while [[ $# -gt 0 ]]; do
  case "$1" in
    --notes) NOTES="${2:?--notes 需要内容}"; shift ;;
    --notes-file) NOTES_FILE="${2:?--notes-file 需要路径}"; shift ;;
    --skip-tests) SKIP_TESTS=true ;;
    --skip-publish) SKIP_PUBLISH=true ;;
    --yes) YES=true ;;
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

# ── 1. 环境与输入 ─────────────────────────────────────────────
step 1/9 "环境与仓库检查"
[[ "$(uname -s)/$(uname -m)" == "Darwin/arm64" ]] || fail "只能在 Apple Silicon Mac 上发布"
for c in git node cargo ossutil curl codesign hdiutil plutil shasum security; do
  command -v "$c" >/dev/null || fail "缺少命令：$c"
done
[[ -x "$TAURI" ]] || fail "缺少签名工具 $TAURI（先执行 npm install）"
[[ -f "$KEY_PATH" ]] || fail "缺少签名私钥：$KEY_PATH"
[[ "$(git branch --show-current)" == "main" ]] || fail "请在 main 分支发布"
[[ -z "$(git status --porcelain)" ]] || fail "工作区有未提交改动，请先提交或清理"

if [[ -z "$VERSION" ]]; then
  [[ -t 0 ]] || fail "非交互模式必须给出版本号"
  read -r -p "当前版本 $(workspace_version)，请输入新版本号：" VERSION
fi
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || fail "版本号必须是 主.次.修订：$VERSION"
git rev-parse -q --verify "refs/tags/v$VERSION" >/dev/null && fail "标签 v$VERSION 已存在"
PUBLISHED="$(remote_version)"
if [[ -n "$PUBLISHED" ]]; then
  version_gt "$VERSION" "$PUBLISHED" || fail "新版本 $VERSION 必须高于线上版本 $PUBLISHED"
fi
echo "线上版本：${PUBLISHED:-（尚未发布）} → 新版本：$VERSION"

OUT="$ROOT/.release/$VERSION"
mkdir -p "$OUT"
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
step 2/9 "写入版本号并提交"
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

# ── 3. 测试 ───────────────────────────────────────────────────
step 3/9 "提交门禁与测试"
if [[ "$SKIP_TESTS" == true ]]; then
  echo "已跳过（--skip-tests）"
else
  scripts/gate.sh >"$OUT/gate.log" 2>&1 || { tail -30 "$OUT/gate.log"; fail "提交门禁未通过（日志：$OUT/gate.log）"; }
  cargo test --workspace --exclude buddy-markdown >"$OUT/test.log" 2>&1 \
    || { grep -E "FAILED|panicked|error" "$OUT/test.log" | head -20; fail "测试失败（日志：$OUT/test.log）"; }
  echo "门禁与测试通过"
fi

# ── 4-5. 构建、组装、签名 ─────────────────────────────────────
step 4/9 "构建 release 二进制"
MACOSX_DEPLOYMENT_TARGET=12.0 cargo build --release --locked -p buddy-app --bin buddy >"$OUT/build.log" 2>&1 \
  || { tail -30 "$OUT/build.log"; fail "构建失败（日志：$OUT/build.log）"; }

step 5/9 "组装 Buddy.app / 更新包 / 安装包 / 源码包并签名"
MAC_DIR="$OUT/macos/aarch64"
mkdir -p "$MAC_DIR" "$OUT/source"
scripts/release/bundle-macos.sh "$VERSION" "$ROOT/target/release/buddy" "$MAC_DIR"
[[ "$(plutil -extract CFBundleShortVersionString raw -o - "$MAC_DIR/Buddy.app/Contents/Info.plist")" == "$VERSION" ]] \
  || fail "Info.plist 版本与 $VERSION 不一致"
git archive --format=tar.gz --prefix="buddy-$VERSION/" -o "$OUT/source/buddy-$VERSION-src.tar.gz" HEAD
UPDATE="$MAC_DIR/Buddy_${VERSION}_aarch64.app.tar.gz"
INSTALLER="$MAC_DIR/Buddy_${VERSION}_aarch64.dmg"
sign "$UPDATE"
sign "$INSTALLER"
REMOTE_DIR="$PREFIX/releases/$VERSION"
node scripts/release/manifest.mjs --version "$VERSION" --notes-file "$OUT/notes.txt" \
  --base-url "$PUBLIC_BASE/$REMOTE_DIR" --dir "$OUT" --output "$OUT/manifest.json"

# ── 6. 上传版本目录 ───────────────────────────────────────────
step 6/9 "上传到 OSS 并公网回读校验"
for f in "$UPDATE" "$UPDATE.sig" "$INSTALLER" "$INSTALLER.sig"; do
  upload "$f" "$REMOTE_DIR/macos/aarch64/$(basename "$f")" "$IMMUTABLE"
done
upload "$OUT/source/buddy-$VERSION-src.tar.gz" "$REMOTE_DIR/source/buddy-$VERSION-src.tar.gz" "$IMMUTABLE"
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

# ── 7. 确认并发布 ─────────────────────────────────────────────
step 7/9 "发布确认"
if [[ "$YES" != true ]]; then
  [[ -t 0 ]] || fail "非交互模式发布需要 --yes"
  read -r -p "即将正式发布 $VERSION，用户将能看到此更新。确认？(y/N) " answer
  [[ "$answer" == "y" || "$answer" == "Y" ]] || fail "已取消；版本目录已上传但未发布"
fi

step 8/9 "更新固定地址 channels/stable.json"
upload "$OUT/manifest.json" "$CHANNEL_KEY" "no-cache"
verify_public "$CHANNEL_URL" "$OUT/manifest.json"

step 9/9 "打本地标签"
git tag -a "v$VERSION" -m "Buddy v$VERSION" "$COMMIT"

INSTALLER_URL="$(node -e 'console.log(require(process.argv[1]).platforms["darwin-aarch64"].installer.url)' "$OUT/manifest.json")"
printf '\n发布完成：Buddy %s（commit %s，标签 v%s 仅本地）\n' "$VERSION" "$COMMIT" "$VERSION"
printf '  版本信息（固定地址）：%s\n' "$CHANNEL_URL"
printf '  安装包下载：%s\n' "$INSTALLER_URL"
