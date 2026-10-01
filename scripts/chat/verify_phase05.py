#!/usr/bin/env python3
"""Phase 05 图片链路拦截验证；运行期间禁止并行编辑源码。

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


UNIT = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib")
IMAGE = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "chat_preview", "--", "--selftest-image")
DRAFT = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "chat_preview", "--", "--selftest-attachments")
ENGINE = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "app_preview", "--", "--selftest-attachments")

# 与生产实现的独立期望：4 张、5MB、纯图片可发送、真实点击与剪贴板结果。
CASES = [
    Case("pure-image-send", "crates/ui/src/chat/page_state.rs", "if !has_content && !has_images {", "if !has_content {", UNIT + ("page_state",), "FAILED"),
    Case("image-size-limit", "crates/ui/src/chat/attachments.rs", "pub const MAX_IMAGE_BYTES: usize = 5 * 1024 * 1024;", "pub const MAX_IMAGE_BYTES: usize = 6 * 1024 * 1024;", UNIT + ("attachments",), "FAILED"),
    Case("image-type-whitelist", "crates/ui/src/chat/attachments.rs", '"image/jpeg" | "image/png" | "image/gif" | "image/webp"', '"image/jpeg" | "image/gif" | "image/webp"', UNIT + ("attachments",), "FAILED"),
    Case("clipboard-route", "crates/ui/src/chat/composer.rs", "if this.paste_from_clipboard(cx) {", "if false {", DRAFT, "FAIL S05-07 T33"),
    Case("image-count-limit", "crates/ui/src/chat/attachments.rs", "pub const MAX_IMAGE_COUNT: usize = 4;", "pub const MAX_IMAGE_COUNT: usize = 5;", DRAFT, "FAIL S05-07 T33"),
    Case("drop-route", "crates/ui/src/chat/composer.rs", "this.add_paths(paths.paths(), cx)", "{ let _ = (this, paths, cx); }", ENGINE, "FAIL S05-07 T34"),
    Case("delete-saved-image", "crates/ui/src/chat/composer_attachments.rs", "spawn_engine(cx, path_task).detach();", "drop(path_task);", ENGINE, "FAIL S05-07 T34"),
    Case("destroyed-composer-cleanup", "crates/ui/src/chat/composer_attachments.rs", "if updated.is_err() {", "if false && updated.is_err() {", ENGINE, "FAIL S05-07 T34"),
    Case("clear-image-draft", "crates/ui/src/chat/router.rs", "let _ = c.take_images(cx);", "let _ = c.images();", ENGINE, "FAIL S05-07 T34"),
    Case("history-image-retry", "crates/ui/src/chat/message_row.rs", "source.remove_asset(cx);", "let _ = &source;", ENGINE, "FAIL S05-07 T34"),
    Case("image-card-route", "crates/ui/src/chat/image_gen.rs", "tool_name == \"generate_image\"", "tool_name == \"disabled_generate_image\"", IMAGE, "FAIL S05-12 T32"),
    Case("image-card-toggle", "crates/ui/src/chat/image_gen.rs", ".on_click(on_toggle)", ".on_click(|_, _, _| {})", IMAGE, "FAIL S05-12 T32"),
    Case("prompt-copy", "crates/ui/src/chat/image_gen.rs", "gpui::ClipboardItem::new_string(copy_prompt.clone())", "gpui::ClipboardItem::new_string(String::new())", IMAGE, "FAIL S05-12 T32"),
    Case("generated-image-placeholder", "crates/ui/src/chat/image_gen_state.rs", ".h(fallback_height)", ".h(px(0.0))", IMAGE, "FAIL S05-12 T32"),
    Case("generated-image-remeasure", "crates/ui/src/chat/transcript.rs", "if before_load != after_load {", "if false && before_load != after_load {", IMAGE, "FAIL S05-12 T32"),
    Case("generated-image-retry", "crates/ui/src/chat/image_gen_state.rs", "cache.update(cx, |cache, cx| cache.remove(&resource, window, cx));", "let _ = (&cache, &resource);", IMAGE, "FAIL S05-12 T32"),
    Case("download-feedback", "crates/ui/src/chat/image_gen_gallery.rs", "Ok(path) => DownloadState::Saved(path)", "Ok(path) => DownloadState::Error(path)", IMAGE, "FAIL S05-12 T32"),
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
    logs = Path(tempfile.mkdtemp(prefix="buddy-phase05-interception-"))
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
