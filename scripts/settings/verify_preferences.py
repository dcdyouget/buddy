#!/usr/bin/env python3
"""S06-05/06 拦截验证；运行期间禁止并行编辑源码，编译错误不算拦截。"""
import argparse
from pathlib import Path
import tempfile
from verify_phase06 import Case, ROOT, run

UNIT = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib", "settings::hotkey::state")
CONFIG = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib", "chat::router::preferences")
GUI = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "settings_preview", "--", "--selftest-preferences")
PROVIDERS = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "settings_preview", "--", "--selftest-providers")
STATE = "crates/ui/src/settings/hotkey_state.rs"
ROUTER = "crates/ui/src/chat/router.rs"
PREF = "crates/ui/src/chat/router_preferences.rs"
VIEW = "crates/ui/src/settings/view/render.rs"
CASES = [
    Case("escape-cancel", STATE, "if is_escape(key) {", "if false {", UNIT, "FAILED"),
    Case("bare-and-fn-rejected", STATE, "if modifier_names(captured_modifiers).is_empty() {", "if false {", UNIT, "FAILED"),
    Case("keydown-modifier-snapshot", STATE, "let captured_modifiers = self.captured_modifiers;", "let captured_modifiers = modifiers;", UNIT, "FAILED"),
    Case("pure-modifier-not-main", STATE, "if is_modifier_key(key) {", "if false {", UNIT, "FAILED"),
    Case("key-name-uppercase", STATE, "key.to_uppercase()", "key.to_string()", UNIT, "FAILED"),
    Case("platform-keycap", STATE, '(true, "CmdOrCtrl") => "⌘".into(),', '(true, "CmdOrCtrl") => "Ctrl".into(),', UNIT, "FAILED"),
    Case("hotkey-delta", PREF, "Self::Hotkey(value) => next.hotkey = value.clone(),", "Self::Hotkey(value) => { let _ = value; },", CONFIG, "FAILED"),
    Case("theme-delta", PREF, "Self::Theme(value) => next.theme = value.clone(),", "Self::Theme(value) => { let _ = value; },", CONFIG, "FAILED"),
    Case("recorded-candidate-event", STATE, "RecordOutcome::Changed(value)", "{ let _ = value; RecordOutcome::None }", GUI, "FAIL S06-05/S06-06"),
    Case("tab-during-recording", VIEW, "if this.hotkey.read(cx).recording() {", "if false {", GUI, "FAIL S06-05/S06-06"),
    Case("theme-click-event", "crates/ui/src/settings/theme_control.rs", "cx.emit(ThemeChanged(theme));", "let _ = theme;", GUI, "FAIL S06-05/S06-06"),
    Case("theme-global-apply", ROUTER,
         "if cx.buddy_theme().appearance != config.theme.clone().into() {",
         "if false {", GUI, "FAIL S06-05/S06-06"),
    Case("theme-restore-on-new-router", ROUTER,
         "        crate::theme_system::set_appearance(config.theme.clone().into(), cx);\n        let conversation",
         "        // Deliberately omit restoring the persisted appearance.\n        let conversation", GUI, "FAIL S06-05/S06-06"),
    Case("preferences-save", PREF,
         "spawn_engine(cx, async move { engine.save_config(to_save).await })",
         "spawn_engine(cx, async move { let _ = (engine, to_save); Ok::<(), String>(()) })", GUI, "FAIL S06-05/S06-06"),
    Case("hotkey-error-visible", "crates/ui/src/settings/hotkey.rs",
         "self.error = Some(message.into());", "let _ = message; self.error = None;", GUI, "FAIL S06-05/S06-06"),
    Case("hotkey-keyboard-start", "crates/ui/src/settings/hotkey.rs",
         'if matches!(event.keystroke.key.as_str(), "enter" | "space") {',
         'if false {', GUI, "FAIL S06-05/S06-06"),
    Case("theme-error-visible", "crates/ui/src/settings/theme_control.rs",
         'self.error = Some(format!("保存外观失败：{}", error.into()));',
         "let _: String = error.into(); self.error = None;", GUI, "FAIL S06-05/S06-06"),
    Case("theme-error-chinese-label", "crates/ui/src/settings/theme_control.rs",
         '保存外观失败：{}', 'Error: {}', GUI, "FAIL S06-05/S06-06"),
    Case("preferences-input-release", "crates/ui/src/settings/view/preferences.rs",
         "let active = self.active && !self.provider_motion.interactive();",
         "let active = self.active;", PROVIDERS, "FAIL S06-02"),
]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--case", action="append", default=[])
    args = parser.parse_args()
    unknown = set(args.case) - {case.name for case in CASES}
    if unknown:
        parser.error(f"未知拦截项：{sorted(unknown)}")
    if args.list:
        for case in CASES:
            print(f"{case.name}: {case.path}")
        return 0
    cases = [case for case in CASES if not args.case or case.name in args.case]
    logs = Path(tempfile.mkdtemp(prefix="buddy-s06-preferences-interception-"))
    print(f"日志目录：{logs}", flush=True)
    for index, command in enumerate(dict.fromkeys(case.command for case in cases)):
        rc, output = run(command, logs / f"baseline-{index}.log")
        marker = "PASS S06-05/S06-06" if command == GUI else ("PASS S06-02" if command == PROVIDERS else "test result: ok.")
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
