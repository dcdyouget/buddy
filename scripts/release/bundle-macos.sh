#!/usr/bin/env bash
# 把已编译的 release 二进制组装为 Buddy.app，并产出更新包（.app.tar.xz）与安装包（.dmg）。
#
# 用法：scripts/release/bundle-macos.sh <版本号> <二进制路径> <输出目录>
# 产出：<输出目录>/Buddy.app、Buddy_<版本>_aarch64.app.tar.xz、Buddy_<版本>_aarch64.dmg
#
# 结构与客户端安装器的校验一一对应（crates/update/src/macos.rs）：
#   CFBundleIdentifier = com.buddy.chat、CFBundleShortVersionString = 清单版本、
#   CFBundleExecutable = buddy、代码签名通过 `codesign --verify --deep --strict`。
set -euo pipefail

VERSION="${1:?缺少版本号}"
BINARY="${2:?缺少二进制路径}"
OUT="${3:?缺少输出目录}"
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
TEMPLATE="$ROOT/apps/buddy/bundle/macos"
APP="$OUT/Buddy.app"
BASE="$OUT/Buddy_${VERSION}_aarch64"

[[ -x "$BINARY" ]] || { echo "找不到可执行文件：$BINARY" >&2; exit 1; }
rm -rf "$APP" "$BASE.app.tar.xz" "$BASE.dmg"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"

sed "s/@VERSION@/$VERSION/g" "$TEMPLATE/Info.plist" >"$APP/Contents/Info.plist"
plutil -lint "$APP/Contents/Info.plist" >/dev/null
# Contents/MacOS 只能放 Mach-O：其他文件的签名存于扩展属性，会被下方 COPYFILE_DISABLE 打包丢弃，
# 导致客户端 `codesign --verify --strict` 拒绝更新。资源一律放 Resources。
cp "$BINARY" "$APP/Contents/MacOS/buddy"
cp "$TEMPLATE/icon.icns" "$APP/Contents/Resources/icon.icns"
# GPL-3.0 随包许可：许可证全文与第三方声明
for f in LICENSE LICENSE-GPL-3.0-or-later LICENSE-APACHE-2.0 THIRD_PARTY_NOTICES.md; do
  cp "$ROOT/$f" "$APP/Contents/Resources/$f"
done

# Ad-hoc 签名：Apple Silicon 必须有签名才能运行（未做 Developer ID 签名与公证）。
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"

# 更新包：xz（系统 bsdtar 原生支持，比 gzip 小得多）。
# COPYFILE_DISABLE 防止 macOS tar 写入 ._ 资源分叉文件，破坏签名校验
COPYFILE_DISABLE=1 /usr/bin/tar -cJf "$BASE.app.tar.xz" --options xz:compression-level=9 -C "$OUT" Buddy.app

# 安装包：应用 + 「应用程序」快捷方式，用户拖入即可安装
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
ditto "$APP" "$STAGE/Buddy.app"
ln -s /Applications "$STAGE/Applications"
# ULMO（LZMA，macOS 10.15+；最低系统 12.0）：比 UDZO 小约 35%
hdiutil create -quiet -volname "Buddy" -srcfolder "$STAGE" -ov -format ULMO "$BASE.dmg"

echo "已生成：$BASE.app.tar.xz"
echo "已生成：$BASE.dmg"
