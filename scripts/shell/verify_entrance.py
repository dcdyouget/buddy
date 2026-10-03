#!/usr/bin/env python3
"""S07-08 入场时序有效拦截；变异期间冻结源码与其他 cargo。"""
import argparse
from pathlib import Path
import sys
import tempfile
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/"settings"))
from verify_phase06 import Case, ROOT, run
UNIT=("cargo","test","-q","-p","buddy-ui","--lib","shell::entrance::tests")
GUI=("cargo","run","-q","-p","buddy-app","--example","shell_preview","--","--selftest-entrance")
CASES=[
    Case("native-animation","crates/ui/src/shell/native.rs","const NS_WINDOW_ANIMATION_NONE: i64 = 2;","const NS_WINDOW_ANIMATION_NONE: i64 = 4;",GUI,"timing=false"),
    Case("followup-frame","crates/ui/src/shell/entrance.rs","self.phase == EntrancePhase::Entering && !reduced_motion","self.phase == EntrancePhase::Entering && reduced_motion",UNIT,"FAILED"),
    Case("reduced-play","crates/ui/src/shell/entrance.rs","fn play_with_preference(&mut self, reduced_motion: bool) {\n        self.phase = if reduced_motion {","fn play_with_preference(&mut self, reduced_motion: bool) {\n        self.phase = if false && reduced_motion {",UNIT,"FAILED"),
    Case("reduced-reset","crates/ui/src/shell/entrance.rs","fn reset_with_preference(&mut self, reduced_motion: bool) {\n        self.phase = if reduced_motion {","fn reset_with_preference(&mut self, reduced_motion: bool) {\n        self.phase = if false && reduced_motion {",UNIT,"FAILED"),
    Case("duration","crates/ui/src/shell/entrance.rs","motion::DURATION_NORMAL as u64","motion::DURATION_NORMAL as u64 + 50",UNIT,"FAILED"),
    Case("curve","crates/ui/src/shell/entrance.rs","let eased = easing::cubic_bezier(motion::EASE_STANDARD)(local);","let eased = local; let _ = easing::cubic_bezier(motion::EASE_STANDARD);",UNIT,"FAILED"),
    Case("keyframe-opacity","crates/ui/src/shell/entrance.rs","0.0, 0.48, 0.18, 0.58","0.0, 0.48, 0.18, 0.48",UNIT,"FAILED"),
    Case("initial-scale","crates/ui/src/shell/entrance.rs","0.0, 0.76, 0.94, 1.006","0.0, 0.76, 0.99, 1.006",UNIT,"FAILED"),
    Case("show-trigger","crates/ui/src/shell/runtime.rs","shell.play_entrance(cx);","// omitted entrance trigger",GUI,"timing=false"),
    Case("expanded-static","crates/ui/src/shell/mod.rs","if self.router.read(cx).page().is_compact() {","if true {",GUI,"expanded_static=false"),
]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case",action="append",default=[])
    args=parser.parse_args()
    if set(args.case)-{c.name for c in CASES}:parser.error("未知拦截项")
    cases=[c for c in CASES if not args.case or c.name in args.case]
    logs=Path(tempfile.mkdtemp(prefix="buddy-s07-entrance-interception-"))
    print(f"日志：{logs}",flush=True)
    for i,cmd in enumerate(dict.fromkeys(c.command for c in cases)):
        rc,out=run(cmd,logs/f"baseline-{i}.log")
        marker="PASS S07-08 T53" if cmd==GUI else "test result: ok"
        if rc or marker not in out or "running 0 tests" in out:
            print(f"基线失败 rc={rc}\n{out}",flush=True);return 1
    results=[]
    for c in cases:
        path=ROOT/c.path;original=path.read_bytes();source=original.decode()
        if source.count(c.before)!=1:
            print(f"FAIL {c.name}: 锚点不唯一",flush=True);results.append(False);continue
        try:
            path.write_text(source.replace(c.before,c.after,1))
            rc,out=run(c.command,logs/f"{c.name}.log")
        finally:path.write_bytes(original)
        summary=next((line for line in out.splitlines() if line.startswith("FAIL S07-08 T53")),"") if c.command==GUI else out
        caught=rc not in (0,124) and c.fail_marker in summary and not any(x in out for x in ("error[E","could not compile","会话已锁定","未获得 CGEventPost 权限"))
        results.append(caught)
        print(f"{'PASS' if caught else 'FAIL'} {c.name}: rc={rc}; 已还原",flush=True)
        if not caught:print(out,flush=True)
    print(f"有效拦截 {sum(results)}/{len(results)}；{logs}",flush=True)
    return 0 if all(results) else 1
if __name__=="__main__":raise SystemExit(main())
