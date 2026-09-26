#!/usr/bin/env bash
# S01-06-9 / S10-05：冷启动与常驻内存基线（v1 与 v2 同一口径）
#
# 用法：scripts/v1-baseline/perf_baseline.sh <可执行文件> [轮数=5] [--long-session]
#
# 每轮：
#   1. 新建沙盒 HOME（dirs::data_dir 读 $HOME → 数据目录落在沙盒，不碰用户真实数据）；
#      复制用户真实 config.json 进去（只读来源）；--long-session 时再放入 1200 条长会话样本
#   2. winwait.swift 启动进程，计时到其第一个窗口出现在屏幕上（window_ms）
#   3. 空闲 5s 后采集：主进程 phys_footprint（footprint）+ 本轮新起的 WebKit 辅助进程 footprint 之和
#   4. 结束进程
# 输出每轮一行，最后一行为中位数。
set -euo pipefail

BIN=${1:?用法: perf_baseline.sh <binary> [runs] [--long-session]}
RUNS=${2:-5}
LONG=0
[[ "${3:-}" == "--long-session" ]] && LONG=1

ROOT=$(cd "$(dirname "$0")/../.." && pwd)
REAL_CONFIG="$HOME/Library/Application Support/com.buddy.chat/config.json"
SAMPLE="$ROOT/target/v1-baseline/long-session"
WORK="$ROOT/target/v1-baseline/perf-home"

if [[ $LONG == 1 && ! -f "$SAMPLE/manifest.json" ]]; then
  python3 "$ROOT/scripts/v1-baseline/gen_long_session.py" >/dev/null
fi

# footprint 输出中 "Footprint: 123 MB" / "phys_footprint: ..." → MB
mb_of() {
  # 形如「zsh [62948]: 64-bit    Footprint: 2336 KB (16384 bytes per page)」
  footprint -p "$1" 2>/dev/null | awk '{for (i=1;i<NF;i++) if ($i=="Footprint:") {v=$(i+1); u=$(i+2); if (u ~ /KB/) v/=1024; if (u ~ /GB/) v*=1024; printf "%.1f\n", v; exit}}'
}

starts=(); mains=(); totals=()
for i in $(seq 1 "$RUNS"); do
  rm -rf "$WORK"
  DATA="$WORK/Library/Application Support/com.buddy.chat"
  mkdir -p "$DATA"
  [[ -f "$REAL_CONFIG" ]] && cp "$REAL_CONFIG" "$DATA/config.json"
  [[ $LONG == 1 ]] && cp "$SAMPLE"/*.json "$DATA/"

  before=$(pgrep -f 'com.apple.WebKit' | sort -n | tr '\n' ' ' || true)
  line=$(HOME="$WORK" swift "$ROOT/scripts/v1-baseline/winwait.swift" "$BIN" 20)
  pid=$(sed -E 's/.*pid=([0-9]+).*/\1/' <<<"$line")
  ms=$(sed -E 's/.*window_ms=([0-9.]+|timeout).*/\1/' <<<"$line")
  sleep 5
  main=$(mb_of "$pid")
  helpers=0
  for h in $(pgrep -f 'com.apple.WebKit' || true); do
    [[ " $before " == *" $h "* ]] && continue
    v=$(mb_of "$h"); helpers=$(awk -v a="$helpers" -v b="${v:-0}" 'BEGIN{print a+b}')
  done
  total=$(awk -v a="${main:-0}" -v b="$helpers" 'BEGIN{print a+b}')
  kill "$pid" 2>/dev/null || true; sleep 1; kill -9 "$pid" 2>/dev/null || true
  echo "run=$i window_ms=$ms main_mb=${main:-?} webkit_mb=$helpers total_mb=$total $line"
  starts+=("$ms"); mains+=("${main:-0}"); totals+=("$total")
done

median() { printf '%s\n' "$@" | sort -n | awk '{a[NR]=$1} END {print (NR%2 ? a[(NR+1)/2] : (a[NR/2]+a[NR/2+1])/2)}'; }
echo "MEDIAN window_ms=$(median "${starts[@]}") main_mb=$(median "${mains[@]}") total_mb=$(median "${totals[@]}") runs=$RUNS long_session=$LONG"
rm -rf "$WORK"
