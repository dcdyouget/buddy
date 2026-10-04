#!/usr/bin/env bash
# 提交门禁：编译无 warning / error，且 vendored markdown 与「上游原文 + GPL patch」一致。
# 用法：scripts/gate.sh >/tmp/gate.log 2>&1; rc=$?   （不要接管道，管道会吞掉退出码）
set -euo pipefail
cd "$(dirname "$0")/.."

# buddy-markdown 只查 lib：vendored zed 源码自带的单元测试依赖 zed 测试设施，无法也无需编译
{ cargo check --workspace --all-targets --exclude buddy-markdown 2>&1; cargo check -p buddy-markdown --lib 2>&1; } \
  >/tmp/buddy-gate.log
if grep -qE '^(warning|error)' /tmp/buddy-gate.log; then
  grep -E '^(warning|error)' -A6 /tmp/buddy-gate.log | head -40
  echo "gate: 编译有 warning / error"; exit 1
fi

# GPL：对 zed markdown 的修改必须完整记录在 patch 中（见 crates/markdown/VENDOR.md）
UPSTREAM=$(ls -d ~/.cargo/git/checkouts/zed-*/290cbcb*/crates/markdown/src 2>/dev/null | head -1)
if [[ -n "$UPSTREAM" ]]; then
  TMP=$(mktemp -d); trap 'rm -rf "$TMP"' EXIT
  cp -R "$UPSTREAM" "$TMP/src"
  (cd "$TMP/src" && patch -s -p1 < "$OLDPWD/crates/markdown/patches/zed-markdown-290cbcb.patch")
  diff -r "$TMP/src" crates/markdown/src >/dev/null || { echo "gate: crates/markdown/src 与 patch 不一致，按 VENDOR.md 重新生成 patch"; exit 1; }
fi
echo "gate: 全部通过"
