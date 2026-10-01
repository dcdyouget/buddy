#!/usr/bin/env python3
"""Phase 06 设置链路拦截验证；运行期间禁止并行编辑源码。

每次只变异一个文件，子进程结束后无论成功或失败都还原原始字节。
编译错误不算有效拦截，必须命中自测的行为失败。日志留在临时目录。
"""

from dataclasses import dataclass
from pathlib import Path
import argparse
import os
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]


@dataclass(frozen=True)
class Case:
    name: str
    path: str
    before: str
    after: str
    command: tuple[str, ...]
    fail_marker: str


UI = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "settings_preview", "--", "--selftest")
CONTROLS = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "settings_preview", "--", "--selftest-controls")

MOTION = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib", "settings::panel::tests::exits_release_input_before_paint_finishes")

CASES = [
    Case("motion-unit", "crates/ui/src/settings/panel.rs", "        self.shown\n", "        !self.shown\n", MOTION, "FAILED"),
    Case("transcript-wheel-occlusion", "crates/ui/src/chat/transcript.rs", "|| !hitbox.should_handle_scroll(window)", "|| false", UI, "FAIL S06-01 T35"),
    Case("settings-route", "crates/ui/src/chat/router.rs", ".child(self.settings.clone())", ".child(div())", UI, "FAIL S06-01 T35"),
    Case("mouse-back", "crates/ui/src/settings/view.rs", ".on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Back)))", ".on_click(cx.listener(|_, _, _, _| {}))", UI, "FAIL S06-01 T35"),
    Case("keyboard-back", "crates/ui/src/settings/view.rs", '\"enter\" | \"space\" if this.back.is_focused(window)', '\"disabled-enter\" | \"disabled-space\" if this.back.is_focused(window)', UI, "FAIL S06-01 T35"),
    Case("settings-scroll", "crates/ui/src/settings/view.rs", "d.overflow_y_scroll()", "d.overflow_hidden()", UI, "FAIL S06-01 T35"),
    Case("overlay-occlusion", "crates/ui/src/settings/view.rs", "d.occlude().capture_key_down", "d.capture_key_down", UI, "FAIL S06-01 T35"),
    Case("exit-input-release", "crates/ui/src/settings/view.rs", "self.active = active;", "self.active = true;", UI, "FAIL S06-01 T35"),
    Case("animation-placement", "crates/ui/src/chat/router.rs", "left(gpui::relative(1.0 - amount))", "left(gpui::relative(0.0))", UI, "FAIL S06-01 T35"),
    Case("draft-retention", "crates/ui/src/chat/router.rs", "view.set_active(shown, cx));", "view.set_active(shown, cx));\n            self.composer.update(cx, |composer, cx| composer.set_draft(\"\", cx));", UI, "FAIL S06-01 T35"),
    Case("select-mouse", "crates/ui/src/settings/select.rs", "self.set_selected(index, cx);", "let _ = (index, cx);", CONTROLS, "FAIL S06-01 T36"),
    Case("select-keyboard", "crates/ui/src/settings/select.rs", "self.set_selected(next, cx);", "let _ = next;", CONTROLS, "FAIL S06-01 T36"),
    Case("select-escape", "crates/ui/src/settings/select.rs", '\"escape\" if self.open => {', '\"disabled-escape\" if self.open => {', CONTROLS, "FAIL S06-01 T36"),
    Case("single-line-paste", "crates/ui/src/settings/controls.rs", ".replace('\\n', \"\")", ".replace('\\n', \"\\n\")", CONTROLS, "FAIL S06-01 T36"),
]


def run(command: tuple[str, ...], log: Path) -> tuple[int, str]:
    env = os.environ.copy()
    env["NO_PROXY"] = "127.0.0.1,localhost"
    with log.open("w") as stream:
        try:
            result = subprocess.run(command, cwd=ROOT, env=env, stdout=stream, stderr=subprocess.STDOUT, timeout=180)
        except subprocess.TimeoutExpired:
            return 124, log.read_text(errors="replace")
    return result.returncode, log.read_text(errors="replace")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--case", action="append", default=[])
    args = parser.parse_args()
    if args.list:
        for case in CASES:
            print(f"{case.name}: {case.path}")
        return 0
    unknown = set(args.case) - {case.name for case in CASES}
    if unknown:
        parser.error(f"未知拦截项：{sorted(unknown)}")
    cases = [case for case in CASES if not args.case or case.name in args.case]
    logs = Path(tempfile.mkdtemp(prefix="buddy-phase06-interception-"))
    print(f"日志目录：{logs}", flush=True)
    commands = list(dict.fromkeys(case.command for case in cases))
    for index, command in enumerate(commands):
        rc, output = run(command, logs / f"baseline-{index}.log")
        if rc or "PASS" not in output and "test result: ok." not in output:
            print(f"基线失败 rc={rc}：{' '.join(command)}\n{output}", flush=True)
            return 1
    results = []
    for case in cases:
        path = ROOT / case.path
        original = path.read_bytes()
        source = original.decode()
        count = source.count(case.before)
        if count != 1:
            print(f"FAIL {case.name}: 变异锚点出现 {count} 次，拒绝修改", flush=True)
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
