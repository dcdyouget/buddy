#!/usr/bin/env python3
"""S07-09/10 原地变异；执行时冻结全部源码、其他 cargo 与 GUI。"""
import argparse
from pathlib import Path
import sys
import tempfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "settings"))
from verify_phase06 import Case, ROOT, run

UNIT = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib", "shell::")
OS = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "autostart_probe")
PREF = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "autostart_preferences_preview", "--", "--selftest")
TRAY = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "tray_preview", "--", "--selftest")
CASES = [
    Case("os-readback", "crates/ui/src/shell/autostart.rs", "Ok(actual) if actual == desired => Ok(()),", "Ok(_actual) => Ok(()),", UNIT, "FAILED"),
    Case("os-rollback", "crates/ui/src/shell/autostart.rs", "backend.apply_raw(before)", "backend.apply_raw(!before)", UNIT, "FAILED"),
    Case("native-registration", "crates/ui/src/shell/autostart.rs", "self.launcher.enable()", "Ok(())", OS, "[S07-10] FAIL"),
    Case("menu-settings", "crates/ui/src/shell/tray.rs", "SETTINGS_ID => Some(MenuAction::Settings)", "SETTINGS_ID => Some(MenuAction::Quit)", UNIT, "FAILED"),
    Case("tray-release", "crates/ui/src/shell/tray.rs", "button_state: MouseButtonState::Up,\n            ..", "button_state: MouseButtonState::Down,\n            ..", UNIT, "FAILED"),
    Case("config-rollback", "crates/ui/src/chat/router_autostart.rs", "backend.set(before)", "Ok::<(), String>(())", PREF, "FAIL S07-10 preferences"),
    Case("config-field", "crates/ui/src/chat/router_autostart.rs", "candidate.auto_start = target;", "candidate.auto_start = before;", PREF, "FAIL S07-10 preferences"),
    Case("menu-readback", "crates/ui/src/shell/tray.rs", "self.autostart.set_checked(enabled);", "self.autostart.set_checked(!enabled);", TRAY, "FAIL S07-09"),
]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", action="append", default=[])
    args = parser.parse_args()
    if set(args.case) - {c.name for c in CASES}:
        parser.error("未知拦截项")
    cases = [c for c in CASES if not args.case or c.name in args.case]
    logs = Path(tempfile.mkdtemp(prefix="buddy-s07-services-interception-"))
    print(f"日志：{logs}", flush=True)
    markers = {UNIT: "test result: ok", OS: "[S07-10] PASS", PREF: "PASS S07-10 preferences", TRAY: "PASS S07-09"}
    for i, command in enumerate(dict.fromkeys(c.command for c in cases)):
        rc, output = run(command, logs / f"baseline-{i}.log")
        if rc or markers[command] not in output or "running 0 tests" in output:
            print(f"基线失败 rc={rc}\n{output}", flush=True)
            return 1
    results = []
    for case in cases:
        path = ROOT / case.path
        original = path.read_bytes()
        source = original.decode()
        if source.count(case.before) != 1:
            print(f"FAIL {case.name}: 锚点不唯一", flush=True)
            results.append(False)
            continue
        try:
            path.write_text(source.replace(case.before, case.after, 1))
            rc, output = run(case.command, logs / f"{case.name}.log")
        finally:
            path.write_bytes(original)
        caught = rc not in (0, 124) and case.fail_marker in output and not any(
            error in output for error in ("error[E", "could not compile", "会话已锁定", "未获得 CGEventPost 权限")
        )
        results.append(caught)
        print(f"{'PASS' if caught else 'FAIL'} {case.name}: rc={rc}; 已还原", flush=True)
        if not caught:
            print(output, flush=True)
    print(f"有效拦截 {sum(results)}/{len(results)}；{logs}", flush=True)
    return 0 if all(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
