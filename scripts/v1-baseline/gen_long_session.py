#!/usr/bin/env python3
"""S01-06-10：生成 1000+ 条消息的长会话样本（v1 存储格式），供 S00-07 / S10-05 做 v1 与 v2 的性能对比。

用法（仓库根目录）：
    python3 scripts/v1-baseline/gen_long_session.py [--out DIR] [--messages N]

默认输出到 target/v1-baseline/long-session/（target/ 已被 gitignore；样本可再生，故不入库）。
输出目录即一个「数据目录」：manifest.json + chunk_NNN.json（每块 100 条，与 v1 CHUNK_SIZE 一致）。
**不含 config.json**：对比时把真实 config.json 复制进去，再让 v1 / v2 指向该目录。

内容按固定随机种子生成（可复现），覆盖真实会话中的渲染负载：
纯文本、长段落、markdown 列表/表格、代码块、思考块、工具调用 + 工具结果、中英混排。
"""

from __future__ import annotations

import json
import random
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CHUNK_SIZE = 100  # v1 storage.rs CHUNK_SIZE
SEED = 20260927

PARAGRAPH = (
    "Buddy 是一个跨平台的 AI 聊天工具。按下全局热键即可唤起一个无边框的轻量窗口，"
    "与模型对话后点击外部即可收起。This sentence mixes English to exercise mixed-script shaping. "
)
CODE = """```rust
fn fibonacci(n: u64) -> u64 {
    match n {
        0 | 1 => n,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}
```"""
TABLE = """| 模型 | 上下文 | 延迟 |
|------|--------|------|
| MiniMax-M3 | 200k | 820ms |
| DeepSeek | 128k | 640ms |"""
LIST = "要点：\n\n1. 第一点说明\n2. 第二点说明，包含 `inline code`\n   - 嵌套项 A\n   - 嵌套项 B\n3. **加粗**与*斜体*"


def assistant_content(rng: random.Random) -> str:
    kind = rng.choice(["short", "long", "code", "table", "list", "mixed"])
    if kind == "short":
        return "好的，已经完成。"
    if kind == "long":
        return PARAGRAPH * rng.randint(4, 12)
    if kind == "code":
        return "示例实现如下：\n\n" + CODE + "\n\n复杂度为指数级。"
    if kind == "table":
        return "对比结果：\n\n" + TABLE
    if kind == "list":
        return LIST
    return PARAGRAPH * 2 + "\n\n" + CODE + "\n\n" + LIST


def build(n: int) -> list[dict]:
    rng = random.Random(SEED)
    msgs: list[dict] = []
    t = 1_780_000_000
    i = 0
    while len(msgs) < n:
        t += rng.randint(5, 600)
        msgs.append({"id": f"u-{i}", "role": "user", "content": f"第 {i} 个问题：{PARAGRAPH[: rng.randint(10, 80)]}",
                     "model_id": None, "created_at": t})
        if rng.random() < 0.15:  # 工具调用轮
            call_id = f"call-{i}"
            msgs.append({"id": f"a-{i}-1", "role": "assistant", "content": "", "model_id": "p1::mock",
                         "created_at": t + 1, "blocks": [],
                         "tool_calls": [{"id": call_id, "name": "websearch",
                                         "arguments": json.dumps({"query": f"查询 {i}"}, ensure_ascii=False)}]})
            msgs.append({"id": f"t-{i}", "role": "tool", "content": "搜索结果：\n" + "- 结果条目\n" * 5,
                         "model_id": None, "created_at": t + 2, "tool_call_id": call_id,
                         "tool_name": "websearch", "is_error": False})
        content = assistant_content(rng)
        blocks = [{"type": "text", "content": content}]
        if rng.random() < 0.3:  # 思考块
            blocks.insert(0, {"type": "thinking", "content": "先分析问题……" * rng.randint(3, 20), "is_open": False})
        msgs.append({"id": f"a-{i}", "role": "assistant", "content": content, "model_id": "p1::mock",
                     "created_at": t + 3, "blocks": blocks})
        i += 1
    return msgs[:n]


def main() -> int:
    out = ROOT / "target" / "v1-baseline" / "long-session"
    n = 1200
    if "--out" in sys.argv:
        out = Path(sys.argv[sys.argv.index("--out") + 1])
    if "--messages" in sys.argv:
        n = int(sys.argv[sys.argv.index("--messages") + 1])
    out.mkdir(parents=True, exist_ok=True)
    for old in out.glob("chunk_*.json"):
        old.unlink()

    msgs = build(n)
    chunks = []
    for k in range(0, len(msgs), CHUNK_SIZE):
        part = msgs[k : k + CHUNK_SIZE]
        name = f"chunk_{k // CHUNK_SIZE + 1:03d}.json"
        (out / name).write_text(json.dumps({"id": name[:-5], "messages": part}, ensure_ascii=False, indent=2), encoding="utf-8")
        chunks.append({"file": name, "count": len(part)})
    (out / "manifest.json").write_text(json.dumps({"chunks": chunks, "total_messages": len(msgs)}, indent=2), encoding="utf-8")

    roles = {r: sum(1 for m in msgs if m["role"] == r) for r in ("user", "assistant", "tool")}
    size = sum(p.stat().st_size for p in out.glob("*.json"))
    print(f"out={out} messages={len(msgs)} chunks={len(chunks)} roles={roles} bytes={size} seed={SEED}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
