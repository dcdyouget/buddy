#!/usr/bin/env python3
"""S06-03 模型设置拦截验证。

该脚本只在显式运行时就地变异一个已确认的源码锚点；每项结束后无论结果如何
都恢复原始字节。编译错误和超时不算有效拦截，必须命中单测或真实 GUI 自测。
运行期间不要编辑产品源码。
"""

from pathlib import Path
import argparse
import sys
import tempfile

try:
    from verify_phase06 import Case, ROOT, run
except ModuleNotFoundError:  # 允许从仓库根目录或其他脚本导入本模块
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from verify_phase06 import Case, ROOT, run


PURE = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib", "settings::model_config_tests")
LATENCY = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib", "settings::model_list")
STATE = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib", "chat::router::config_save")
MODEL_GUI = (
    "cargo",
    "run",
    "-q",
    "-p",
    "buddy-app",
    "--example",
    "settings_preview",
    "--",
    "--selftest-models",
)


CASES = [
    Case(
        "model-default",
        "crates/ui/src/settings/model_config.rs",
        "            next.selected_model_id = model_id.clone();",
        "            next.selected_model_id.clear();",
        PURE,
        "FAILED",
    ),
    Case(
        "model-toggle-fallback",
        "crates/ui/src/settings/model_config.rs",
        "            if !selected_is_enabled {",
        "            if false {",
        PURE,
        "FAILED",
    ),
    Case(
        "model-provider-scope",
        "crates/ui/src/settings/model_config.rs",
        ".position(|provider| provider.id == provider_id)",
        ".position(|provider| provider.id != provider_id)",
        PURE,
        "FAILED",
    ),
    Case(
        "model-context-exact",
        "crates/ui/src/settings/model_config.rs",
        "    if !options.contains(&value) {",
        "    if false {",
        PURE,
        "FAILED",
    ),
    Case(
        "model-vision",
        "crates/ui/src/settings/model_config.rs",
        "            next.models[model_index].supports_vision = *value;",
        "            next.models[model_index].supports_vision = false;",
        PURE,
        "FAILED",
    ),
    Case(
        "model-image-protocol",
        "crates/ui/src/settings/model_config.rs",
        '            if next.providers[provider_index].provider_type == "openai_compatible" {',
        "            if false {",
        PURE,
        "FAILED",
    ),
    Case(
        "model-latency-boundaries",
        "crates/ui/src/settings/model_list/helpers.rs",
        "    if value < 500 {",
        "    if value < 400 {",
        LATENCY,
        "FAILED",
    ),
    Case(
        "save-committed-base",
        "crates/ui/src/chat/router_config_save.rs",
        "        self.committed = config.clone();",
        "        self.committed = AppConfig::default();",
        STATE,
        "FAILED",
    ),
    Case(
        "save-optimistic-selection",
        "crates/ui/src/chat/router_config_save.rs",
        "            if *pending > sequence {",
        "            if *pending < sequence {",
        STATE,
        "FAILED",
    ),
    Case(
        "save-committed-read",
        "crates/ui/src/chat/router_config_save.rs",
        "        self.committed.clone()",
        "        AppConfig::default()",
        STATE,
        "FAILED",
    ),
    Case(
        "model-control-mouse",
        "crates/ui/src/settings/model_list/helpers.rs",
        ".when(active, |d| d.on_click(click))",
        ".when(active, |d| d)",
        MODEL_GUI,
        "FAIL S06-03 T41",
    ),
    Case(
        "model-default-button",
        "crates/ui/src/settings/model_list/helpers.rs",
        "                    cx.emit(ModelEdit::SetDefault(model_id.clone()));",
        "                    let _ = model_id;",
        MODEL_GUI,
        "FAIL S06-03 T41",
    ),
    Case(
        "model-context-reset",
        "crates/ui/src/settings/model_list/mod.rs",
        "                    select.reset_options(labels, selected, cx);",
        "                    let _ = (labels, selected);",
        MODEL_GUI,
        "FAIL S06-03 T41",
    ),
    Case(
        "model-save-error",
        "crates/ui/src/settings/model_list/mod.rs",
        "        self.error = Some(message);",
        "        self.error = None;",
        MODEL_GUI,
        "FAIL S06-03 T42",
    ),
    Case(
        "model-save-failure-sync",
        "crates/ui/src/settings/model_list/mod.rs",
        "        // Selects update their displayed selection on user input. Restore the\n        // committed value after a failed write, just as controlled v1 inputs do.\n        self.sync_models(cx);",
        "        // Selects update their displayed selection on user input. Restore the\n        // committed value after a failed write, just as controlled v1 inputs do.\n        let _ = cx;",
        MODEL_GUI,
        "FAIL S06-03 T42",
    ),
    Case(
        "model-tab-navigation",
        "crates/ui/src/settings/model_list/mod.rs",
        "        for model in &self.config.models {",
        "        for model in self.config.models.iter().rev() {",
        MODEL_GUI,
        "FAIL S06-03 T42",
    ),
    Case(
        "model-success-publish",
        "crates/ui/src/chat/router_settings.rs",
        "                    let _ = router.update(cx, |router, cx| router.publish_config(visible, cx));\n                    let _ = settings.update(cx, |view, cx| view.set_model_saving(false, cx));",
        "                    let _ = router.update(cx, |router, cx| router.publish_config(AppConfig::default(), cx));\n                    let _ = settings.update(cx, |view, cx| view.set_model_saving(false, cx));",
        MODEL_GUI,
        "FAIL S06-03 T41",
    ),
    Case(
        "model-provider-input-release",
        "crates/ui/src/settings/view.rs",
        "            .update(cx, |models, cx| models.set_active(false, cx));",
        "            .update(cx, |models, cx| models.set_active(true, cx));",
        ("cargo", "run", "-q", "-p", "buddy-app", "--example", "settings_preview", "--", "--selftest-providers"),
        "FAIL S06-02",
    ),
    Case(
        "model-add-focus-scroll",
        "crates/ui/src/settings/view/render.rs",
        '                                if id != "back" {',
        '                                if id != "back" && id != "add" {',
        ("cargo", "run", "-q", "-p", "buddy-app", "--example", "settings_preview", "--", "--selftest-providers"),
        "FAIL S06-02",
    ),

    Case("model-context-write", "crates/ui/src/settings/model_config.rs",
         "            next.models[model_index].context_window = *value;",
         "            next.models[model_index].context_window = *value + 1;", PURE, "FAILED"),
    Case("model-latency-error-threshold", "crates/ui/src/settings/model_list/helpers.rs",
         "    } else if value < 1500 {", "    } else if value < 1600 {", LATENCY, "FAILED"),
    Case("model-default-keyboard", "crates/ui/src/settings/model_list/helpers.rs",
         "                cx.emit(ModelEdit::SetDefault(key_model_id.clone()));",
         "                let _ = &key_model_id;", MODEL_GUI, "FAIL S06-03 T42"),

]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--list", action="store_true")
    parser.add_argument("--case", action="append", default=[])
    args = parser.parse_args()
    names = {case.name for case in CASES}
    unknown = set(args.case) - names
    if unknown:
        parser.error(f"未知拦截项：{sorted(unknown)}")
    if args.list:
        for case in CASES:
            print(f"{case.name}: {case.path}")
        return 0

    cases = [case for case in CASES if not args.case or case.name in args.case]
    logs = Path(tempfile.mkdtemp(prefix="buddy-s06-03-interception-"))
    print(f"日志目录：{logs}", flush=True)

    commands = list(dict.fromkeys(case.command for case in cases))
    for index, command in enumerate(commands):
        rc, output = run(command, logs / f"baseline-{index}.log")
        if rc or ("PASS" not in output and "test result: ok." not in output):
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
        intercepted = (
            rc not in (0, 124)
            and case.fail_marker in output
            and "error[E" not in output
            and "could not compile" not in output
        )
        results.append(intercepted)
        print(f"{'PASS' if intercepted else 'FAIL'} {case.name}: rc={rc}; 源码已还原", flush=True)
        if not intercepted:
            print(output, flush=True)

    print(f"拦截 {sum(results)}/{len(results)}；日志：{logs}", flush=True)
    return 0 if all(results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
