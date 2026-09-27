#!/usr/bin/env bash
# 提交闸门：全部检查串联，任一失败即非零退出。
#
# 用法：scripts/gate.sh && git commit ...
#
# 为什么需要：曾两次把检查与提交写在同一行却用 `;` 连接，检查失败后提交照样执行
# （S02-02 状态不一致、S03-05 预览程序圆角违规）。统一入口避免手写检查链出错。
set -euo pipefail
cd "$(dirname "$0")/.."

python3 scripts/check-discipline.py >/tmp/buddy-gate.log 2>&1 || { cat /tmp/buddy-gate.log; echo "gate: 纪律检查失败"; exit 1; }
python3 scripts/check-discipline.py --self-test >/tmp/buddy-gate.log 2>&1 || { cat /tmp/buddy-gate.log; echo "gate: 拦截验证失败"; exit 1; }
# buddy-markdown 只查 lib：vendored zed 源码自带的单元测试依赖 zed 测试设施与被 stub 的 language，
# 无法也无需编译（不改 vendored 源码以保持与 zed 原文一致，见 crates/markdown/Cargo.toml）
{ cargo check --workspace --all-targets --exclude buddy-markdown 2>&1; cargo check -p buddy-markdown --lib 2>&1; } \
  | tee /tmp/buddy-gate.log | grep -qE '^(warning|error)' && { grep -E '^(warning|error)' -A6 /tmp/buddy-gate.log | head -40; echo "gate: 编译有 warning / error"; exit 1; }
(cd src-tauri && cargo check >/tmp/buddy-gate.log 2>&1) || { tail -20 /tmp/buddy-gate.log; echo "gate: v1 编译失败"; exit 1; }
echo "gate: 全部通过"
