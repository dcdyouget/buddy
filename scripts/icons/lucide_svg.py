#!/usr/bin/env python3
"""从 v1 所用的 lucide-react 生成 SVG 文件（图形与 v1 逐字一致，ISC 许可）。

用法：python3 scripts/icons/lucide_svg.py <图标名>...   # 如 circle-alert x chevron-down
输出：crates/ui/assets/icons/<图标名>.svg；已存在且内容相同则不改动。
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "node_modules/lucide-react/dist/esm/icons"
OUT = ROOT / "crates/ui/assets/icons"
HEAD = ('<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24" fill="none" '
        'stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">')


def svg(name: str) -> str:
    text = (SRC / f"{name}.mjs").read_text(encoding="utf-8")
    body = re.search(r"const __iconNode = \[(.*?)\];", text, re.S).group(1)
    parts = []
    for tag, attrs in re.findall(r'\["(\w+)",\s*\{(.*?)\}\]', body, re.S):
        pairs = [(k, v) for k, v in re.findall(r'(\w+):\s*"([^"]*)"', attrs) if k != "key"]
        parts.append("<" + tag + "".join(f' {k}="{v}"' for k, v in pairs) + "/>")
    if not parts:
        raise SystemExit(f"{name}: 未解析到图形")
    return HEAD + "".join(parts) + "</svg>\n"


def main() -> None:
    for name in sys.argv[1:]:
        out = OUT / f"{name}.svg"
        content = svg(name)
        if out.exists() and out.read_text(encoding="utf-8") == content:
            print(f"未变 {out.relative_to(ROOT)}")
            continue
        out.write_text(content, encoding="utf-8")
        print(f"写入 {out.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
