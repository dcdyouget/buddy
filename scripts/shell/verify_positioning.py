#!/usr/bin/env python3
"""S07-06 定位与原生几何拦截；运行期间禁止其他源码编辑或 cargo。

每项只改一个锚点，finally 恢复原始字节。编译失败、权限失败、超时不计拦截。
"""

import argparse
from pathlib import Path
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "settings"))
from verify_phase06 import Case, ROOT, run

GEOMETRY = "crates/ui/src/shell/positioning.rs"
CONTROLLER = "crates/ui/src/shell/positioning_controller.rs"
UNIT = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib", "shell::positioning")
GUI = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "shell_preview", "--", "--selftest-positioning")
FAIL = "FAIL S07-06"

CASES = [
    Case("coordinate-origin", GEOMETRY, "primary_top - (rect.origin.y + rect.size.height)", "primary_top - rect.origin.y", UNIT, "FAILED"),
    Case("clamp-minimum", GEOMETRY, "let min = start + margin;", "let min = 0.0;", UNIT, "FAILED"),
    Case("clamp-maximum", GEOMETRY, "start + extent - target - margin", "start + extent - margin", UNIT, "FAILED"),
    Case("oversized-clamp", GEOMETRY, ".max(min);", ";", UNIT, "FAILED"),
    Case("horizontal-anchor", GEOMETRY, "start.origin.x + (start.size.width - target_size.width) / 2.0", "start.origin.x", UNIT, "FAILED"),
    Case("bottom-anchor", GEOMETRY, "start.origin.y + start.size.height - target_size.height", "start.origin.y", UNIT, "FAILED"),
    Case("saved-position", GEOMETRY, "self.saved(&screen.key)", "None", UNIT, "FAILED"),
    Case("display-fallback", GEOMETRY, ".find_map(|key| displays.iter().find(|display| display.key == key))", ".next().and_then(|key| displays.iter().find(|display| display.key == key))", UNIT, "FAILED"),
    Case("position-save", GEOMETRY, "self.positions.insert(screen.key.clone(), origin);", "let _ = (screen, origin);", UNIT, "FAILED"),
    Case("save-debounce", CONTROLLER, "Duration::from_millis(160)", "Duration::from_millis(1)", GUI, FAIL),
    Case("save-observer", CONTROLLER, "|shell, window, _| save_current(shell, window)", "|_, _, _| {}", GUI, FAIL),
    Case("show-restore", "crates/ui/src/shell/runtime.rs", "positioning_controller::restore(handle, true, cx).await?;", "let _ = handle;", GUI, FAIL),
    Case("page-geometry", CONTROLLER, "let origin = positioning::bottom_anchored(snapshot.rect, size, &snapshot.screen);", "let origin = snapshot.rect.origin;", GUI, FAIL),
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--case", action="append", default=[])
    args = parser.parse_args()
    known = {case.name for case in CASES}
    if set(args.case) - known:
        parser.error("未知拦截项")
    if args.list:
        for case in CASES:
            print(f"{case.name}: {case.path}")
        return 0
    cases = [case for case in CASES if not args.case or case.name in args.case]
    logs = Path(tempfile.mkdtemp(prefix="buddy-s07-positioning-interception-"))
    print(f"日志目录：{logs}", flush=True)
    for index, command in enumerate(dict.fromkeys(case.command for case in cases)):
        rc, output = run(command, logs / f"baseline-{index}.log")
        marker = "PASS S07-06" if command == GUI else "test result: ok."
        if rc or marker not in output or "running 0 tests" in output:
            print(f"基线失败 rc={rc}：{output}", flush=True)
            return 1
    results = []
    for case in cases:
        path = ROOT / case.path
        original = path.read_bytes()
        source = original.decode()
        if source.count(case.before) != 1:
            print(f"FAIL {case.name}: 锚点不唯一，拒绝修改", flush=True)
            results.append(False)
            continue
        try:
            path.write_text(source.replace(case.before, case.after, 1))
            rc, output = run(case.command, logs / f"{case.name}.log")
        finally:
            path.write_bytes(original)
        intercepted = (case.command != GUI or "T51：" in output) and rc not in (0, 124) and case.fail_marker in output and "error[E" not in output and "could not compile" not in output and not any(marker in output for marker in ("权限预检未通过", "会话已锁定", "未获得 CGEventPost 权限"))
        results.append(intercepted)
        print(f"{'PASS' if intercepted else 'FAIL'} {case.name}: rc={rc}; 源码已还原", flush=True)
        if not intercepted:
            print(output, flush=True)
    print(f"拦截 {sum(results)}/{len(results)}；日志：{logs}", flush=True)
    return 0 if all(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
