#!/usr/bin/env python3
"""S06-04 MCP 配置契约拦截：运行期间禁止编辑源码，编译错误不计有效拦截。"""

import argparse
from pathlib import Path
import tempfile
from verify_phase06 import Case, ROOT, run

MODEL = ("cargo", "test", "-q", "-p", "buddy-engine", "--lib", "models::mcp")
STORAGE = ("cargo", "test", "-q", "-p", "buddy-engine", "--lib", "storage::config_files")
UI = ("cargo", "test", "-q", "-p", "buddy-ui", "--lib", "settings::model_mcp_tests")
MCP_PATH = "crates/engine/src/models/mcp.rs"
CASES = [
    Case("args-retained", MCP_PATH, "    pub args: Vec<String>,", "    #[serde(skip)]\n    pub args: Vec<String>,", MODEL, "FAILED"),
    Case("env-retained", MCP_PATH, "    pub env: std::collections::HashMap<String, String>,", "    #[serde(skip)]\n    pub env: std::collections::HashMap<String, String>,", MODEL, "FAILED"),
    Case("headers-retained", MCP_PATH, "    pub headers: std::collections::HashMap<String, String>,", "    #[serde(skip)]\n    pub headers: std::collections::HashMap<String, String>,", MODEL, "FAILED"),
    Case("enabled-reconnect-default", MCP_PATH, "fn default_true() -> bool {\n    true\n}", "fn default_true() -> bool {\n    false\n}", MODEL, "FAILED"),
    Case("timeout-default", MCP_PATH, "fn default_timeout_secs() -> u32 {\n    30\n}", "fn default_timeout_secs() -> u32 {\n    60\n}", MODEL, "FAILED"),
    Case("stdio-required-command", MCP_PATH, 'if self.command.as_deref().unwrap_or("").is_empty() {', "if false {", MODEL, "FAILED"),
    Case("sse-required-url", MCP_PATH, 'if self.url.as_deref().unwrap_or("").is_empty() {', "if false {", MODEL, "FAILED"),
    Case("stdio-whitespace-contract", MCP_PATH, 'if self.command.as_deref().unwrap_or("").is_empty() {', 'if self.command.as_deref().unwrap_or("").trim().is_empty() {', MODEL, "FAILED"),
    Case("sse-whitespace-contract", MCP_PATH, 'if self.url.as_deref().unwrap_or("").is_empty() {', 'if self.url.as_deref().unwrap_or("").trim().is_empty() {', MODEL, "FAILED"),
    Case("id-required", MCP_PATH, "    pub id: String,", "    #[serde(default)]\n    pub id: String,", MODEL, "FAILED"),
    Case("name-required", MCP_PATH, "    pub name: String,", "    #[serde(default)]\n    pub name: String,", MODEL, "FAILED"),
    Case("storage-retains-mcp", "crates/engine/src/storage/config_files.rs", "    let mut config = config.clone();", "    let mut config = config.clone();\n    config.mcp_servers.clear();", STORAGE, "FAILED"),
    Case("legacy-mcp-default", "crates/engine/src/models/config.rs", "    #[serde(default)]\n    pub mcp_servers: Vec<McpServerConfig>,", "    pub mcp_servers: Vec<McpServerConfig>,", STORAGE, "FAILED"),
    Case("model-edit-retains-mcp", "crates/ui/src/settings/model_config.rs", "    let mut next = config.clone();", "    let mut next = config.clone();\n    next.mcp_servers.clear();", UI, "FAILED"),
    Case("provider-merge-retains-mcp", "crates/ui/src/settings/provider_merge.rs", "    let mut next = config.clone();", "    let mut next = config.clone();\n    next.mcp_servers.clear();", UI, "FAILED"),
    Case("invalid-read-does-not-write", "crates/engine/src/storage/config_files.rs",
         '        log::warn!("配置文件损坏，使用默认配置: {error}");',
         '        log::warn!("配置文件损坏，使用默认配置: {error}");\n        let _ = fs::write(dir.join("config.json"), "{}");', STORAGE, "FAILED"),

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
    logs = Path(tempfile.mkdtemp(prefix="buddy-s06-04-interception-"))
    print(f"日志目录：{logs}", flush=True)
    for index, command in enumerate(dict.fromkeys(case.command for case in cases)):
        rc, output = run(command, logs / f"baseline-{index}.log")
        if rc or "test result: ok." not in output or "running 0 tests" in output:
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
