#!/usr/bin/env python3
"""S07-11/12 有效拦截；变异期间冻结全部源码和其他 cargo / GUI。"""
import argparse
from pathlib import Path
import sys
import tempfile
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "settings"))
from verify_phase06 import Case, ROOT, run

PROBE = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "lifecycle_probe")
UNIT = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib", "shell::")
GUI = ("cargo", "run", "-q", "-p", "buddy-app", "--bin", "buddy", "--", "--selfcheck-window")
WAKE = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "wake_preview", "--", "--selftest")
APP = ("cargo", "run", "-q", "-p", "buddy-app", "--example", "lifecycle_app_probe")
CORE = "crates/ui/src/shell/hotkey/core.rs"
core_source = (ROOT / CORE).read_text()
rearm_start = core_source.index("    pub(crate) fn rearm(")
rearm_end = core_source.index("    pub(crate) fn shutdown(", rearm_start)
rearm_block = core_source[rearm_start:rearm_end]
CASES = [
    Case("owner-lock", "crates/ui/src/shell/lifecycle/instance.rs", "let result = unsafe { libc::flock(lock.file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };", "let result = 0; let _ = lock;", PROBE, "secondary 越过 owner 锁"),
    Case("stale-socket", "crates/ui/src/shell/lifecycle/instance.rs", "let _ = fs::remove_file(path);\n                thread::sleep(BIND_RETRY_DELAY);", "let _ = path;\n                thread::sleep(BIND_RETRY_DELAY);", PROBE, "绑定单实例 socket失败"),
    Case("socket-cleanup", "crates/ui/src/shell/lifecycle/instance.rs", "let _ = fs::remove_file(&self.socket_path);", "let _ = &self.socket_path;", PROBE, "owner 正常 drop 后 socket 未清理"),
    Case("exit-lock", "crates/ui/src/shell/lifecycle/instance.rs", "std::mem::forget(self);", "drop(self);", PROBE, "prepare_process_exit 后锁已释放，产生并行 owner"),
    Case("rearm-unregister", "crates/ui/src/shell/hotkey/core.rs", "if let Err(reason) = self.backend.unregister(old) {", "if let Err(reason) = Ok::<(), String>(()) {", UNIT, "FAILED"),
    Case("rearm-pressed", CORE, rearm_block, rearm_block.replace("self.pressed.clear();", "// keep stale pressed"), UNIT, "FAILED"),
    Case("diagnostic-shadow", "crates/ui/src/shell/selfcheck.rs", "&& !snapshot.has_shadow", "&& true", UNIT, "FAILED"),
    Case("wake-registration", "crates/ui/src/shell/runtime.rs", ".rearm()", ".unregister_all()", WAKE, "唤醒恢复后没有有效的系统热键"),
    Case("quit-hook", "crates/ui/src/shell/lifecycle.rs", "guard.prepare_process_exit();", "std::mem::forget(guard);", APP, "真实 on_app_quit 后 socket 未清理"),
    Case("native-shadow", "crates/ui/src/shell/native.rs", "let _: () = objc::msg_send![native, setHasShadow: objc::runtime::NO];", "let _: () = objc::msg_send![native, setHasShadow: objc::runtime::YES];", GUI, "FAIL S07-12"),
]

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--case", action="append", default=[])
    args = parser.parse_args()
    if set(args.case) - {c.name for c in CASES}: parser.error("未知拦截项")
    cases = [c for c in CASES if not args.case or c.name in args.case]
    logs = Path(tempfile.mkdtemp(prefix="buddy-s07-lifecycle-interception-"))
    print(f"日志：{logs}", flush=True)
    for i, command in enumerate(dict.fromkeys(c.command for c in cases)):
        rc, output = run(command, logs / f"baseline-{i}.log")
        marker = {PROBE: "[S07-11] PASS", UNIT: "test result: ok", GUI: "PASS S07-12", WAKE: "PASS S07-11 wake", APP: "[S07-11] PASS"}[command]
        if rc or marker not in output or "running 0 tests" in output:
            print(f"基线失败 rc={rc}\n{output}", flush=True); return 1
    results = []
    for case in cases:
        path = ROOT / case.path
        original = path.read_bytes(); source = original.decode()
        if source.count(case.before) != 1:
            print(f"FAIL {case.name}: 锚点不唯一", flush=True); results.append(False); continue
        try:
            path.write_text(source.replace(case.before, case.after, 1))
            rc, output = run(case.command, logs / f"{case.name}.log")
        finally:
            path.write_bytes(original)
        caught = rc not in (0, 124) and case.fail_marker in output and not any(
            marker in output for marker in ("error[E", "could not compile", "BLOCKED S07-12", "超时")
        )
        results.append(caught)
        print(f"{'PASS' if caught else 'FAIL'} {case.name}: rc={rc}; 已还原", flush=True)
        if not caught: print(output, flush=True)
    print(f"有效拦截 {sum(results)}/{len(results)}；{logs}", flush=True)
    return 0 if all(results) else 1

if __name__ == "__main__": raise SystemExit(main())
