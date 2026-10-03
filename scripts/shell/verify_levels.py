#!/usr/bin/env python3
"""S07-05 全工作区与普通层级拦截；运行期间禁止其他源码编辑或 cargo。

每项只改一个锚点，finally 恢复原始字节。编译失败、权限失败、超时不计拦截。
"""

import argparse
from pathlib import Path
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "settings"))
from verify_phase06 import Case, ROOT, run

GUI = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "shell_preview", "--", "--selftest-level")
MENU_GUI = GUI[:-1] + ("--selftest-behavior",)
MENU_FAIL = "T48 同应用模型菜单保持主窗 level=0：false"
FAIL = "FAIL S07-05"
CASES = [
    Case("native-focus-restore", "crates/ui/src/shell/visibility.rs", "if state.is_visible && state.is_key {\n                self.restore_normal_level()?;", "if state.is_visible && state.is_key {\n                let _ = state;", GUI, FAIL),
    Case("same-app-focus-level", "crates/ui/src/shell/visibility.rs", " && !state.app_is_active", " && (!state.app_is_active || !state.is_key)", MENU_GUI, MENU_FAIL),
    Case("external-deactivation-level", "crates/ui/src/shell/visibility.rs", "setLevel: -1i64", "setLevel: 0i64", GUI, FAIL),
    Case("all-spaces-collection", "crates/ui/src/shell/native.rs", "let _: () = objc::msg_send![native, setLevel: NS_NORMAL_WINDOW_LEVEL];", "let _: () = objc::msg_send![native, setLevel: NS_NORMAL_WINDOW_LEVEL];\n            let _: () = objc::msg_send![native, setCollectionBehavior: 0u64];", GUI, FAIL),
    Case("normal-show-level", "crates/ui/src/shell/visibility.rs", "setLevel: 0i64", "setLevel: 3i64", MENU_GUI, MENU_FAIL),
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
    logs = Path(tempfile.mkdtemp(prefix="buddy-s07-level-interception-"))
    print(f"日志目录：{logs}", flush=True)
    for index, command in enumerate(dict.fromkeys(case.command for case in cases)):
        rc, output = run(command, logs / f"baseline-{index}.log")
        marker = "PASS S07-05" if command == GUI else "PASS S07-03/S07-04 behavior"
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
        intercepted = rc not in (0, 124) and case.fail_marker in output and ("T50:" in output if case.command == GUI else MENU_FAIL in output) and "error[E" not in output and "could not compile" not in output and not any(marker in output for marker in ("权限预检未通过", "会话已锁定", "未获得 CGEventPost 权限"))
        results.append(intercepted)
        print(f"{'PASS' if intercepted else 'FAIL'} {case.name}: rc={rc}; 源码已还原", flush=True)
        if not intercepted:
            print(output, flush=True)
    print(f"拦截 {sum(results)}/{len(results)}；日志：{logs}", flush=True)
    return 0 if all(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
