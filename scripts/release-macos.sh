#!/usr/bin/env bash

set -Eeuo pipefail

readonly SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
readonly OSS_BUCKET="${OSS_BUCKET:-buddy-release}"
readonly OSS_ENDPOINT="${OSS_ENDPOINT:-https://oss-cn-beijing.aliyuncs.com}"
readonly OSS_PUBLIC_BASE="${OSS_PUBLIC_BASE:-https://buddy-release.oss-cn-beijing.aliyuncs.com}"
readonly OSS_PREFIX="${OSS_PREFIX:-buddy}"

SKIP_UPLOAD=false
ALLOW_UNTAGGED=false
VERSION=""

usage() {
  cat <<'EOF'
用法：
  npm run release:mac -- <版本号> [--skip-upload] [--allow-untagged]

示例：
  npm run release:mac -- 1.4.0

选项：
  --skip-upload      只构建并整理制品，不上传 OSS
  --allow-untagged   允许当前提交没有对应 v<版本号> 标签，仅用于本地验证

环境变量：
  TAURI_SIGNING_PRIVATE_KEY           Updater 私钥路径或内容
  TAURI_SIGNING_PRIVATE_KEY_PASSWORD  Updater 私钥密码；未设置时会安全提示输入
  OSS_BUCKET                          默认 buddy-release
  OSS_ENDPOINT                        默认 https://oss-cn-beijing.aliyuncs.com
  OSS_PUBLIC_BASE                     默认 Bucket 官方 HTTPS 域名
  OSS_PREFIX                          默认 buddy

上传前请安装并配置 ossutil 2.x，Region 使用 cn-beijing。
EOF
}

fail() {
  printf '错误：%s\n' "$*" >&2
  exit 1
}

info() {
  printf '\n==> %s\n' "$*"
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || fail "缺少命令：$1"
}

read_json_version() {
  node -e 'const fs = require("node:fs"); console.log(JSON.parse(fs.readFileSync(process.argv[1], "utf8")).version);' "$1"
}

read_cargo_version() {
  awk '
    /^\[package\]$/ { in_package = 1; next }
    in_package && /^version[[:space:]]*=/ {
      gsub(/^version[[:space:]]*=[[:space:]]*"|"[[:space:]]*$/, "")
      print
      exit
    }
  ' "$1"
}

find_new_artifact() {
  local directory="$1"
  local pattern="$2"
  local marker="$3"
  local matches=()
  local path

  while IFS= read -r -d '' path; do
    matches+=("$path")
  done < <(find "$directory" -type f -name "$pattern" -newer "$marker" -print0 2>/dev/null)

  if [[ "${#matches[@]}" -ne 1 ]]; then
    fail "在 $directory 中预期找到 1 个新生成的 $pattern，实际找到 ${#matches[@]} 个"
  fi

  printf '%s\n' "${matches[0]}"
}

upload_file() {
  local source="$1"
  local destination="$2"
  local cache_control="$3"

  ossutil cp "$source" "$destination" \
    --endpoint "$OSS_ENDPOINT" \
    -f \
    --cache-control "$cache_control"
}

while [[ "$#" -gt 0 ]]; do
  case "$1" in
    -h|--help)
      usage
      exit 0
      ;;
    --skip-upload)
      SKIP_UPLOAD=true
      ;;
    --allow-untagged)
      ALLOW_UNTAGGED=true
      ;;
    v[0-9]*|[0-9]*)
      [[ -z "$VERSION" ]] || fail "只能指定一个版本号"
      VERSION="${1#v}"
      ;;
    *)
      fail "未知参数：$1"
      ;;
  esac
  shift
done

[[ -n "$VERSION" ]] || {
  usage
  exit 1
}
[[ "$VERSION" =~ ^[0-9]+\.[0-9]+\.[0-9]+([+-][0-9A-Za-z.-]+)?$ ]] || fail "版本号不是有效 SemVer：$VERSION"

cd "$PROJECT_ROOT"

require_command git
require_command node
require_command npm
require_command cargo
require_command curl
[[ "$(uname -s)" == "Darwin" ]] || fail "此脚本只能在 macOS 运行"

readonly PACKAGE_VERSION="$(read_json_version "$PROJECT_ROOT/package.json")"
readonly TAURI_VERSION="$(read_json_version "$PROJECT_ROOT/src-tauri/tauri.conf.json")"
readonly CARGO_VERSION="$(read_cargo_version "$PROJECT_ROOT/src-tauri/Cargo.toml")"

[[ "$PACKAGE_VERSION" == "$VERSION" ]] || fail "package.json 版本为 $PACKAGE_VERSION，期望 $VERSION"
[[ "$TAURI_VERSION" == "$VERSION" ]] || fail "tauri.conf.json 版本为 $TAURI_VERSION，期望 $VERSION"
[[ "$CARGO_VERSION" == "$VERSION" ]] || fail "Cargo.toml 版本为 $CARGO_VERSION，期望 $VERSION"

if [[ -n "$(git status --porcelain --untracked-files=normal)" ]]; then
  git status --short
  fail "发布前工作区必须保持干净，请先提交当前改动"
fi

if [[ "$ALLOW_UNTAGGED" != true ]] && ! git tag --points-at HEAD | grep -Fxq "v$VERSION"; then
  fail "当前提交缺少标签 v$VERSION；请先创建标签，或仅在本地验证时使用 --allow-untagged"
fi

if [[ -z "${TAURI_SIGNING_PRIVATE_KEY:-}" ]]; then
  readonly DEFAULT_KEY_PATH="$HOME/.tauri/buddy.key"
  [[ -f "$DEFAULT_KEY_PATH" ]] || fail "未设置 TAURI_SIGNING_PRIVATE_KEY，且找不到 $DEFAULT_KEY_PATH"
  export TAURI_SIGNING_PRIVATE_KEY="$DEFAULT_KEY_PATH"
fi

if [[ -z "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" ]]; then
  [[ -t 0 ]] || fail "非交互环境必须设置 TAURI_SIGNING_PRIVATE_KEY_PASSWORD"
  read -r -s -p "请输入 Updater 私钥密码：" TAURI_SIGNING_PRIVATE_KEY_PASSWORD
  printf '\n'
  export TAURI_SIGNING_PRIVATE_KEY_PASSWORD
fi

if [[ "$SKIP_UPLOAD" != true ]]; then
  require_command ossutil
fi

info "安装锁定依赖并执行测试"
npm ci
npm run test:run
cargo test --manifest-path "$PROJECT_ROOT/src-tauri/Cargo.toml"

readonly RELEASE_ROOT="$PROJECT_ROOT/.release/$VERSION/macos"
mkdir -p "$RELEASE_ROOT"

build_architecture() {
  local rust_target="$1"
  local release_arch="$2"
  local updater_platform="$3"
  local target_root="$PROJECT_ROOT/src-tauri/target/$rust_target/release/bundle"
  local output_dir="$RELEASE_ROOT/$release_arch"
  local marker="$output_dir/.build-started"

  mkdir -p "$output_dir"
  touch "$marker"

  info "构建 macOS $release_arch"
  npm run tauri -- build --target "$rust_target" --bundles app,dmg

  local dmg_source
  local updater_source
  local signature_source
  dmg_source="$(find_new_artifact "$target_root/dmg" '*.dmg' "$marker")"
  updater_source="$(find_new_artifact "$target_root/macos" '*.app.tar.gz' "$marker")"
  signature_source="$(find_new_artifact "$target_root/macos" '*.app.tar.gz.sig' "$marker")"

  local artifact_base="Buddy_${VERSION}_${release_arch}"
  local dmg_output="$output_dir/$artifact_base.dmg"
  local updater_output="$output_dir/$artifact_base.app.tar.gz"
  local signature_output="$updater_output.sig"
  local fragment_output="$output_dir/$updater_platform.json"

  cp "$dmg_source" "$dmg_output"
  cp "$updater_source" "$updater_output"
  cp "$signature_source" "$signature_output"

  local remote_dir="$OSS_PREFIX/releases/$VERSION/macos/$release_arch"
  local updater_url="$OSS_PUBLIC_BASE/$remote_dir/$(basename "$updater_output")"

  node "$SCRIPT_DIR/create-release-fragment.mjs" \
    --version "$VERSION" \
    --platform "$updater_platform" \
    --url "$updater_url" \
    --signature-file "$signature_output" \
    --output "$fragment_output"

  if [[ "$SKIP_UPLOAD" == true ]]; then
    info "已跳过上传：$output_dir"
    return
  fi

  info "上传 macOS $release_arch 制品到 OSS"
  upload_file "$dmg_output" "oss://$OSS_BUCKET/$remote_dir/$(basename "$dmg_output")" "public,max-age=31536000,immutable"
  upload_file "$updater_output" "oss://$OSS_BUCKET/$remote_dir/$(basename "$updater_output")" "public,max-age=31536000,immutable"
  upload_file "$signature_output" "oss://$OSS_BUCKET/$remote_dir/$(basename "$signature_output")" "public,max-age=31536000,immutable"
  upload_file "$fragment_output" "oss://$OSS_BUCKET/$OSS_PREFIX/releases/$VERSION/manifests/$updater_platform.json" "no-cache"

  curl --fail --silent --show-error --head --retry 3 "$updater_url" >/dev/null
  info "已验证公开下载：$updater_url"
}

build_architecture "aarch64-apple-darwin" "aarch64" "darwin-aarch64"

info "macOS $VERSION 构建完成"
printf '本地制品：%s\n' "$RELEASE_ROOT"
if [[ "$SKIP_UPLOAD" != true ]]; then
  printf 'OSS 目录：oss://%s/%s/releases/%s/\n' "$OSS_BUCKET" "$OSS_PREFIX" "$VERSION"
  printf '下一步：完成 Windows 构建后再生成并发布 stable/latest.json。\n'
fi
