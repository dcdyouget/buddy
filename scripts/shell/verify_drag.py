#!/usr/bin/env python3
"""S07-07 拖动 / 字形隔离变异；执行期间冻结全部源码和其他 cargo。"""
import argparse
from pathlib import Path
import sys
import tempfile
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "settings"))
from verify_phase06 import Case, ROOT, run

GUI = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "shell_preview", "--", "--selftest-drag")
APP = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "app_preview", "--", "--selftest")
CASES = [
    Case("inactive-settings", "crates/ui/src/chat/drag.rs", "div().when(enabled, |d| {", "div().when(true || enabled, |d| {", APP, "FAIL S05-18 T25"),
    Case("native-drag", "crates/ui/src/chat/drag.rs", "window.start_window_move();", "let _ = window;", GUI, "edge=false"),
    Case("body-blank", "crates/ui/src/chat/message_row.rs", "drag::invoke(&source, window)", "{ let _ = (&source, window); }", GUI, "blank=false"),
    Case("glyph-selection", "crates/markdown/src/markdown.rs", "&& position_result.is_err()", "&& true", GUI, "selection=false"),
    Case("scroll-preserved", "crates/ui/src/chat/transcript.rs", "let _ = weak.update(cx, |t, cx| t.on_wheel(delta, cx));", "let _ = (&weak, delta);", GUI, "scroll=false"),
]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case",action="append",default=[])
    args=parser.parse_args()
    if set(args.case)-{case.name for case in CASES}: parser.error("未知拦截项")
    cases=[c for c in CASES if not args.case or c.name in args.case]
    logs=Path(tempfile.mkdtemp(prefix="buddy-s07-drag-interception-"))
    print(f"日志：{logs}",flush=True)
    for i,command in enumerate(dict.fromkeys(c.command for c in cases)):
        rc,output=run(command,logs/f"baseline-{i}.log")
        marker="PASS S07-07 T52" if command==GUI else "PASS S05-18 T25"
        if rc or marker not in output:
            print(f"基线失败 rc={rc}\n{output}",flush=True);return 1
    results=[]
    for case in cases:
        path=ROOT/case.path
        original=path.read_bytes();source=original.decode()
        if source.count(case.before)!=1:
            print(f"FAIL {case.name}: 锚点不唯一",flush=True);results.append(False);continue
        try:
            path.write_text(source.replace(case.before,case.after,1))
            rc,output=run(case.command,logs/f"{case.name}.log")
        finally:
            path.write_bytes(original)
        summary=next((line for line in output.splitlines() if line.startswith("FAIL S07-07 T52" if case.command==GUI else "FAIL S05-18 T25")),"")
        caught=rc not in (0,124) and case.fail_marker in summary and not any(marker in output for marker in ("error[E","could not compile","会话已锁定","未获得 CGEventPost 权限"))
        results.append(caught)
        print(f"{'PASS' if caught else 'FAIL'} {case.name}: rc={rc}; 已还原",flush=True)
        if not caught:print(output,flush=True)
    print(f"有效拦截 {sum(results)}/{len(results)}；{logs}",flush=True)
    return 0 if all(results) else 1
if __name__=="__main__":raise SystemExit(main())
