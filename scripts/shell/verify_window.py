#!/usr/bin/env python3
"""S07-01/02 窗口拦截：运行期间禁止并行编辑源码，编译失败不算拦截。"""

from pathlib import Path
import argparse
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "settings"))
from verify_phase06 import Case, ROOT, run

CONFIG = "crates/ui/src/shell/config.rs"
SIZING = "crates/ui/src/shell/sizing.rs"
SHELL = "crates/ui/src/shell/mod.rs"
NATIVE = "crates/ui/src/shell/native.rs"
UNIT = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib", "shell::")
GUI = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "shell_preview", "--", "--selftest-window")
FAIL = "FAIL S07-01/S07-02"

CASES = [
    Case("compact-size", CONFIG, "pub const COMPACT_SIZE: LogicalSize = LogicalSize::new(560, 60);", "pub const COMPACT_SIZE: LogicalSize = LogicalSize::new(560, 62);", UNIT, "FAILED"),
    Case("conversation-size", CONFIG, "pub const CONVERSATION_SIZE: LogicalSize = LogicalSize::new(750, 500);", "pub const CONVERSATION_SIZE: LogicalSize = LogicalSize::new(751, 500);", GUI, FAIL),
    Case("settings-size", CONFIG, "pub const SETTINGS_SIZE: LogicalSize = LogicalSize::new(760, 640);", "pub const SETTINGS_SIZE: LogicalSize = LogicalSize::new(762, 640);", GUI, FAIL),
    Case("minimum-size", CONFIG, "pub const MIN_WINDOW_SIZE: LogicalSize = LogicalSize::new(360, 60);", "pub const MIN_WINDOW_SIZE: LogicalSize = LogicalSize::new(359, 60);", UNIT, "FAILED"),
    Case("resizable-option", CONFIG, "resizable: true,", "resizable: false,", GUI, FAIL),
    Case("focus-option", CONFIG, "focus: false,", "focus: true,", GUI, FAIL),
    Case("premature-show", CONFIG, 'show: cfg!(not(target_os = "macos")),', "show: true,", GUI, FAIL),
    Case("opaque-background", CONFIG, "window_background: WindowBackgroundAppearance::Transparent,", "window_background: WindowBackgroundAppearance::Opaque,", GUI, FAIL),
    Case("zero-minimum-validation", CONFIG, "if self.min_size.width == 0 || self.min_size.height == 0 {", "if false {", UNIT, "FAILED"),
    Case("initial-minimum-validation", CONFIG, "if self.initial_size.width < self.min_size.width\n            || self.initial_size.height < self.min_size.height", "if false", UNIT, "FAILED"),
    Case("page-size-dispatch", SIZING, "Page::Conversation | Page::Streaming => CONVERSATION_SIZE,", "Page::Conversation | Page::Streaming => SETTINGS_SIZE,", GUI, FAIL),
    Case("resize-subscription", SHELL, "window.resize(target.to_gpui());", "let _ = target;", GUI, FAIL),
    Case("settings-origin", SHELL, "settings_origin = Some(from);", "settings_origin = None;", GUI, FAIL),
    Case("compact-return", SIZING, "return Some(COMPACT_SIZE);", "return None;", GUI, FAIL),
    Case("preserve-user-size", SIZING, "    None\n}\n", "    Some(page_size(to))\n}\n", GUI, FAIL),
    Case("native-decoration", NATIVE, "NS_TITLED | NS_CLOSABLE | NS_MINIATURIZABLE | NS_RESIZABLE", "NS_CLOSABLE | NS_MINIATURIZABLE | NS_RESIZABLE", GUI, FAIL),
    Case("native-resizable", NATIVE, "if self.resizable {", "if false {", GUI, FAIL),
    Case("native-shadow", NATIVE, "setHasShadow: objc::runtime::NO", "setHasShadow: objc::runtime::YES", GUI, FAIL),
    Case("native-level", NATIVE, "const NS_NORMAL_WINDOW_LEVEL: i64 = 0;", "const NS_NORMAL_WINDOW_LEVEL: i64 = 3;", GUI, FAIL),
    Case("native-radius", NATIVE, "setCornerRadius: m::RADIUS_XL as f64", "setCornerRadius: m::RADIUS_LG as f64", GUI, FAIL),
    Case("native-clipping", NATIVE, "setMasksToBounds: YES", "setMasksToBounds: objc::runtime::NO", GUI, FAIL),
    Case("native-show", NATIVE, "let _: () = objc::msg_send![native, orderFrontRegardless];", "let _ = native;", GUI, FAIL),
    Case("native-focus", NATIVE, "orderFrontRegardless", "makeKeyAndOrderFront: std::ptr::null_mut::<objc::runtime::Object>()", GUI, FAIL),
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--case", action="append", default=[])
    args = parser.parse_args()
    names = {case.name for case in CASES}
    if set(args.case) - names:
        parser.error("未知拦截项")
    if args.list:
        for case in CASES:
            print(f"{case.name}: {case.path}")
        return 0
    cases = [case for case in CASES if not args.case or case.name in args.case]
    logs = Path(tempfile.mkdtemp(prefix="buddy-s07-window-interception-"))
    print(f"日志目录：{logs}", flush=True)
    for index, command in enumerate(dict.fromkeys(case.command for case in cases)):
        rc, output = run(command, logs / f"baseline-{index}.log")
        marker = "PASS S07-01/S07-02" if command == GUI else "test result: ok."
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
        intercepted = rc not in (0, 124) and case.fail_marker in output and "error[E" not in output and "could not compile" not in output
        results.append(intercepted)
        print(f"{'PASS' if intercepted else 'FAIL'} {case.name}: rc={rc}; 源码已还原", flush=True)
        if not intercepted:
            print(output, flush=True)
    print(f"拦截 {sum(results)}/{len(results)}；日志：{logs}", flush=True)
    return 0 if all(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
