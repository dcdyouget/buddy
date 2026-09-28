#!/usr/bin/env python3
"""从 v1 所用的 lucide-react 生成 SVG 文件（图形与 v1 逐字一致，ISC 许可）。

用法：
  python3 scripts/icons/lucide_svg.py <图标名>...   # 生成（如 circle-alert x chevron-down）
  python3 scripts/icons/lucide_svg.py --check        # 核对：仓库中全部 lucide 图标与源数据逐字一致（提交门禁调用）
输出：crates/ui/assets/icons/<图标名>.svg；已存在且内容相同则不改动。

曾有缺陷：多行书写的图元未被解析，生成的「发送」「设置」图标缺笔画且未报错（S05-10 发现）。
现在解析出的图元数必须等于源数据中的条目数，否则报错。
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
    for tag, attrs in re.findall(r'\[\s*"(\w+)",\s*\{(.*?)\}\s*\]', body, re.S):
        pairs = [(k, v) for k, v in re.findall(r'(\w+):\s*"([^"]*)"', attrs) if k != "key"]
        parts.append("<" + tag + "".join(f' {k}="{v}"' for k, v in pairs) + "/>")
    expected = len(re.findall(r'\[\s*"\w+",', body))
    if not parts or len(parts) != expected:
        raise SystemExit(f"{name}: 解析出 {len(parts)} 个图元，源数据有 {expected} 个")
    return HEAD + "".join(parts) + "</svg>\n"


# 非 lucide 来源（Buddy 自绘），核对时跳过
NOT_LUCIDE = {"streaming-star"}


def check() -> None:
    bad = []
    names = sorted(p.stem for p in OUT.glob("*.svg") if p.stem not in NOT_LUCIDE)
    for name in names:
        if (OUT / f"{name}.svg").read_text(encoding="utf-8") != svg(name):
            bad.append(name)
    if bad:
        raise SystemExit(f"图标与 lucide 源数据不一致：{bad}（重新生成：python3 scripts/icons/lucide_svg.py {' '.join(bad)}）")
    print(f"lucide 图标一致：{len(names)} 个")


def main() -> None:
    if sys.argv[1:] == ["--check"]:
        check()
        return
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
