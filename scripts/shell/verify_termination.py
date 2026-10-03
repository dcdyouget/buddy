#!/usr/bin/env python3
"""原地延迟真实配置写入，验证 macOS 原生退出等待；运行时冻结源码和其他 cargo。"""
from pathlib import Path
import sys
import tempfile
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'settings'))
from verify_phase06 import ROOT, run


def main():
    preferences = ROOT / 'crates/ui/src/chat/router_preferences.rs'
    lifecycle = ROOT / 'crates/ui/src/shell/lifecycle.rs'
    originals = {p: p.read_bytes() for p in (preferences, lifecycle)}
    logs = Path(tempfile.mkdtemp(prefix='buddy-quit-interception-'))
    command = ('cargo', 'run', '-q', '-p', 'buddy-app', '--example', 'lifecycle_app_probe')
    worker = '        cx.spawn(async move |router, cx| {'
    wait = 'Some(pending) => pending.await,'
    print(f'日志：{logs}', flush=True)
    try:
        source = originals[preferences].decode()
        quit_source = originals[lifecycle].decode()
        assert source.count(worker) == 1 and quit_source.count(wait) == 1
        preferences.write_text(source.replace(worker, worker + '\n            cx.background_executor().timer(std::time::Duration::from_millis(700)).await;', 1))
        rc, output = run(command, logs / 'delayed-baseline.log')
        if rc or '[S07-11] PASS' not in output:
            print(f'FAIL 延迟保存基线 rc={rc}\n{output}', flush=True)
            return 1
        lifecycle.write_text(quit_source.replace(wait, 'Some(pending) => drop(pending),', 1))
        rc, output = run(command, logs / 'skip-drain.log')
        caught = rc not in (0, 124) and 'native quit 后主题未落盘' in output and not any(x in output for x in ('error[E', 'could not compile', '超时'))
        print(f'{"PASS" if caught else "FAIL"} native-quit-drain rc={rc}', flush=True)
        if not caught:
            print(output, flush=True)
        return 0 if caught else 1
    finally:
        for path, content in originals.items():
            path.write_bytes(content)
        print('已还原配置延迟和退出变异', flush=True)


if __name__ == '__main__':
    raise SystemExit(main())
