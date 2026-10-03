#!/usr/bin/env python3
"""S07-07 Markdown 命中边界拦截（GPUI 事件，不冒充原生窗口拖动）。"""
import argparse
from pathlib import Path
import sys
import tempfile
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/"settings"))
from verify_phase06 import Case,ROOT,run
GUI=("cargo","run","-q","-p","buddy-app","--example","markdown_preview","--","--selftest")
CASES=[
    Case("blank-hit","crates/markdown/src/markdown.rs","&& position_result.is_err()","&& false",GUI,"blank=false"),
    Case("glyph-hit","crates/markdown/src/markdown.rs","&& position_result.is_err()","&& true",GUI,"text=false"),
    Case("code-button","crates/ui/src/markdown/code_block.rs",".on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())",".on_mouse_down(MouseButton::Left, |_, _, _| {})",GUI,"copy=false"),
    Case("image-hit","crates/markdown/src/markdown.rs","wrapper\n                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())","wrapper\n                .on_mouse_down(MouseButton::Left, { let handler = self.on_blank_mouse_down.clone(); move |_, window, cx| { if let Some(handler) = &handler { handler(window); } cx.stop_propagation(); } })",GUI,"image(unlinked/linked)=false/false"),
]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case",action="append",default=[])
    args=parser.parse_args()
    if set(args.case)-{c.name for c in CASES}:parser.error("未知拦截项")
    cases=[c for c in CASES if not args.case or c.name in args.case]
    logs=Path(tempfile.mkdtemp(prefix="buddy-s07-md-drag-interception-"))
    print(f"日志：{logs}",flush=True)
    rc,out=run(GUI,logs/"baseline.log")
    if rc or "PASS S07-07 Markdown" not in out:
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
        summary=next((line for line in out.splitlines() if line.startswith("T08:")),"")
        caught=rc not in (0,124) and "FAIL S07-07 Markdown" in out and c.fail_marker in summary and "error[E" not in out and "could not compile" not in out
        results.append(caught)
        print(f"{'PASS' if caught else 'FAIL'} {c.name}: rc={rc}; 已还原",flush=True)
        if not caught:print(out,flush=True)
    print(f"有效拦截 {sum(results)}/{len(results)}；{logs}",flush=True)
    return 0 if all(results) else 1
if __name__=="__main__":raise SystemExit(main())
