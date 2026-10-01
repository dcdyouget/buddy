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

PROVIDERS = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "settings_preview", "--", "--selftest-providers")
PROVIDER_UNIT = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib", "settings::provider")
MASK_UNIT = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib", "text_area::tests")

MOTION = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib", "settings::panel::tests::exits_release_input_before_paint_finishes")

CASES = [
    Case("motion-unit", "crates/ui/src/settings/panel.rs", "        self.shown\n", "        !self.shown\n", MOTION, "FAILED"),
    Case("transcript-wheel-occlusion", "crates/ui/src/chat/transcript.rs", "|| !hitbox.should_handle_scroll(window)", "|| false", UI, "FAIL S06-01 T35"),
    Case("settings-route", "crates/ui/src/chat/router.rs", ".child(self.settings.clone())", ".child(div())", UI, "FAIL S06-01 T35"),
    Case("mouse-back", "crates/ui/src/settings/view/render.rs", ".on_click(cx.listener(|_, _, _, cx| cx.emit(SettingsEvent::Back)))", ".on_click(cx.listener(|_, _, _, _| {}))", UI, "FAIL S06-01 T35"),
    Case("keyboard-back", "crates/ui/src/settings/view/render.rs", '\"enter\" | \"space\" if this.back.is_focused(window)', '\"disabled-enter\" | \"disabled-space\" if this.back.is_focused(window)', UI, "FAIL S06-01 T35"),
    Case("settings-scroll", "crates/ui/src/settings/view/render.rs", "d.overflow_y_scroll()", "d.overflow_hidden()", UI, "FAIL S06-01 T35"),
    Case("overlay-occlusion", "crates/ui/src/settings/view/render.rs", "d.occlude().capture_key_down", "d.capture_key_down", UI, "FAIL S06-01 T35"),
    Case("exit-input-release", "crates/ui/src/settings/view.rs", "self.active = active;", "self.active = true;", UI, "FAIL S06-01 T35"),
    Case("animation-placement", "crates/ui/src/chat/router.rs", "left(gpui::relative(1.0 - amount))", "left(gpui::relative(0.0))", UI, "FAIL S06-01 T35"),
    Case("draft-retention", "crates/ui/src/chat/router.rs", "view.set_active(shown, cx));", "view.set_active(shown, cx));\n            self.composer.update(cx, |composer, cx| composer.set_draft(\"\", cx));", UI, "FAIL S06-01 T35"),
    Case("select-mouse", "crates/ui/src/settings/select.rs", "self.set_selected(index, cx);", "let _ = (index, cx);", CONTROLS, "FAIL S06-01 T36"),
    Case("select-keyboard", "crates/ui/src/settings/select.rs", "self.set_selected(next, cx);", "let _ = next;", CONTROLS, "FAIL S06-01 T36"),
    Case("select-escape", "crates/ui/src/settings/select.rs", '\"escape\" if self.open => {', '\"disabled-escape\" if self.open => {', CONTROLS, "FAIL S06-01 T36"),
    Case("single-line-paste", "crates/ui/src/settings/controls.rs", ".replace('\\n', \"\")", ".replace('\\n', \"\\n\")", CONTROLS, "FAIL S06-01 T36"),
    Case("provider-preset-data", "crates/ui/src/settings/provider_presets.rs", 'url: "https://api.deepseek.com",', 'url: "https://wrong.invalid",', PROVIDER_UNIT, "FAILED"),
    Case("provider-stale-fetch", "crates/ui/src/settings/provider_form_model.rs", "        if revision != self.revision {\n            return;\n        }\n        self.busy = Busy::Idle;\n        match result", "        if false { return; }\n        self.busy = Busy::Idle;\n        match result", PROVIDERS, "FAIL S06-02"),
    Case("provider-invalidate", "crates/ui/src/settings/provider_form.rs", "        self.models.clear();", "        // 故意保留旧连接模型", PROVIDER_UNIT, "FAILED"),
    Case("provider-empty-response", "crates/ui/src/settings/provider_form_model.rs", "self.error = Some(EMPTY_MODELS_ERROR.to_string());", "self.error = None;", PROVIDERS, "FAIL S06-02"),
    Case("provider-default-multi-select", "crates/ui/src/settings/provider_form_model.rs", "self.selected = models.iter().map(|model| model.id.clone()).collect();", "self.selected.clear();", PROVIDERS, "FAIL S06-02"),
    Case("provider-latency", "crates/ui/src/settings/provider_form_model.rs", "model.latency_ms = Some(latency);", "let _ = latency;", PROVIDERS, "FAIL S06-02"),
    Case("provider-anthropic-capability", "crates/ui/src/settings/provider_form_model.rs", 'if self.protocol == "anthropic" {\n                        model.supports_image_generation = false;', 'if false {\n                        model.supports_image_generation = false;', PROVIDER_UNIT, "FAILED"),
    Case("provider-custom-compat", "crates/ui/src/settings/provider_form_model.rs", "self.compat_edited.then(|| self.compat_config())", "None", PROVIDER_UNIT, "FAILED"),
    Case("provider-merge-preserves-config", "crates/ui/src/settings/provider_merge.rs", "let mut next = config.clone();", "let mut next = AppConfig::default();", PROVIDER_UNIT, "FAILED"),
    Case("provider-merge-scoping", "crates/ui/src/settings/provider_merge.rs", "let scoped_id = scoped_model_id(&provider_id, &raw_id);", "let scoped_id = raw_id.clone();", PROVIDER_UNIT, "FAILED"),
    Case("provider-custom-protocol", "crates/ui/src/settings/provider_form.rs", "self.protocol = self.custom_protocol.clone();", 'self.protocol = "openai_compatible".to_string();', PROVIDER_UNIT, "FAILED"),
    Case("provider-default-model", "crates/ui/src/settings/provider_merge.rs", "next.selected_model_id = scoped_model_id(&provider_id, raw_id);", "next.selected_model_id = raw_id.to_string();", PROVIDER_UNIT, "FAILED"),
    Case("password-unicode-map", "crates/ui/src/text_area.rs", "source_to_display[end] = display_end;", "source_to_display[end] = display_start;", MASK_UNIT, "FAILED"),
    Case("password-mask-char", "crates/ui/src/text_area.rs", "display.push('•');", "display.push('x');", MASK_UNIT, "FAILED"),
    Case("single-line-wrap", "crates/ui/src/text_area.rs", "(input.enter_mode != EnterMode::SingleLine).then_some(width)", "Some(width)", CONTROLS, "FAIL S06-02 T39"),
    Case("single-line-scroll", "crates/ui/src/text_area.rs", "if area.enter_mode == EnterMode::SingleLine {\n                (content_w - bounds.size.width).max(px(0.))", "if false {\n                (content_w - bounds.size.width).max(px(0.))", CONTROLS, "FAIL S06-02 T39"),
    Case("disabled-field-input", "crates/ui/src/text_area.rs", "self.enabled = enabled;", "self.enabled = true;", CONTROLS, "FAIL S06-02 T39"),
    Case("provider-save-failure", "crates/ui/src/chat/router_settings.rs", "panel.save_failed(error, cx)", "panel.set_saving(false, cx)", PROVIDERS, "FAIL S06-02"),
    Case("provider-return-conversation", "crates/ui/src/chat/router_settings.rs", "pages.set_page(Page::Conversation)", "pages.set_page(Page::Settings)", PROVIDERS, "FAIL S06-02"),
    Case("provider-latency-raw-id", "crates/ui/src/settings/provider_form_model.rs", "let raw_id = raw_model_id(model).to_string();", "let raw_id = model.id.clone();", PROVIDER_UNIT, "FAILED"),
    Case("provider-stale-latency", "crates/ui/src/settings/provider_form_model.rs", "        if revision != self.revision {\n            return;\n        }\n        self.busy = Busy::Idle;\n        let Some(model)", "        if false { return; }\n        self.busy = Busy::Idle;\n        let Some(model)", PROVIDER_UNIT, "FAILED"),
    Case("provider-preserve-last-models", "crates/ui/src/settings/provider_form_model.rs", "self.error = Some(EMPTY_MODELS_ERROR.to_string());", "self.models.clear(); self.error = Some(EMPTY_MODELS_ERROR.to_string());", PROVIDER_UNIT, "FAILED"),
    Case("provider-context-exact", "crates/ui/src/settings/provider/panel_ops.rs", "if !options.contains(&value) {", "if false {", PROVIDER_UNIT, "FAILED"),
    Case("provider-publish-on-failure", "crates/ui/src/chat/router_settings.rs", "                Err(error) => {\n                    let _ = panel.update", "                Err(error) => {\n                    let _ = router.update(cx, |router, cx| router.set_config(candidate, cx));\n                    let _ = panel.update", PROVIDERS, "FAIL S06-02"),
    Case("provider-key-visibility", "crates/ui/src/settings/provider/panel.rs", "self.form.show_key = !self.form.show_key;", "self.form.show_key = false;", PROVIDERS, "FAIL S06-02"),
    Case("provider-model-checkbox", "crates/ui/src/settings/provider/render/models.rs", "this.form.toggle_model(&id);", "let _ = &id;", PROVIDERS, "FAIL S06-02"),
    Case("provider-keyboard-focus", "crates/ui/src/settings/provider/render.rs", "this.focus_next(window, cx, event.keystroke.modifiers.shift);", "let _ = (window, event);", PROVIDERS, "FAIL S06-02"),
    Case("provider-keyboard-activation", "crates/ui/src/settings/provider/panel.rs", 'id if id.starts_with("preset-") => self.select_preset(&id[7..], cx),', 'id if id.starts_with("preset-") => {},', PROVIDERS, "FAIL S06-02"),
    Case("provider-menu-viewport", "crates/ui/src/settings/select.rs", "let open_above = self.last_bounds.get().is_some_and(|bounds| {", "let open_above = false && self.last_bounds.get().is_some_and(|bounds| {", PROVIDERS, "FAIL S06-02"),
    Case("provider-thinking-binding", "crates/ui/src/settings/provider/panel_init.rs", "this.form.set_thinking_format(value);", "let _ = value;", PROVIDERS, "FAIL S06-02"),
    Case("provider-max-tokens-binding", "crates/ui/src/settings/provider/panel_init.rs", "this.form.set_max_tokens_field(value);", "let _ = value;", PROVIDERS, "FAIL S06-02"),
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
