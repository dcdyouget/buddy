#!/usr/bin/env python3
"""S01-06-8：从 v1 代码真值（src/styles/global.css）提取设计令牌，并与 docs/design/design-tokens.md 对照。

用法（仓库根目录）：
    python3 scripts/v1-baseline/extract_tokens.py [--css REF:PATH] > docs/evidence/v1-baseline/tokens.md

默认读取 tag `v1-final` 中的 CSS（基线应对应退路版本，而非工作区）。
输出为 Markdown：完整令牌表（浅色 / 深色）+ 与 design-tokens.md 的差异清单。
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CSS_REF = "v1-final:src/styles/global.css"
# design-tokens.md 已于 S03-04 退役删除；对照其最后一个版本（首次入库 e91bbc3，此后未改）
DOC_REF = "e91bbc3:docs/design/design-tokens.md"


def read_css(ref: str) -> str:
    return subprocess.run(
        ["git", "show", ref], cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout


def block(css: str, selector: str) -> str:
    """取出 `selector {` 起、括号配平处止的块体。"""
    start = css.index(selector + " {")
    i = css.index("{", start) + 1
    depth = 1
    j = i
    while depth:
        if css[j] == "{":
            depth += 1
        elif css[j] == "}":
            depth -= 1
        j += 1
    return css[i : j - 1]


def variables(body: str) -> dict[str, str]:
    body = re.sub(r"/\*.*?\*/", "", body, flags=re.S)
    out = {}
    for m in re.finditer(r"(--[a-z0-9-]+)\s*:\s*([^;]+);", body):
        out[m.group(1)] = " ".join(m.group(2).split())
    return out


def resolve(value: str, table: dict[str, str], depth: int = 0) -> str:
    """把 `var(--x)` 按同主题的变量表展开到底（最多 10 层，防环）。"""
    if depth > 10:
        return value
    return re.sub(
        r"var\((--[a-z0-9-]+)\)",
        lambda m: resolve(table.get(m.group(1), m.group(0)), table, depth + 1),
        value,
    )


def norm(v: str) -> str:
    v = v.strip().strip("`").lower()
    v = re.sub(r"\s+", "", v)
    v = re.sub(r"(?<![0-9])0\.", ".", v)  # 0.82 ≡ .82
    return v


def doc_tokens(text: str) -> dict[str, tuple[str, str | None]]:
    """design-tokens.md 中声明的 {token: (light_or_value, dark|None)}。"""
    out: dict[str, tuple[str, str | None]] = {}
    for line in text.splitlines():
        m = re.match(r"^\|\s*`(--[a-z0-9-]+)`\s*\|\s*`([^`]+)`\s*\|\s*(?:`([^`]+)`)?", line)
        if m:
            out[m.group(1)] = (m.group(2), m.group(3))
            continue
        for m in re.finditer(r"(--[a-z0-9-]+)\s*:\s*([^;]+);", line):
            out[m.group(1)] = (" ".join(m.group(2).split()), None)
        for m in re.finditer(r"(--[a-z0-9-]+)=(\d+)", line):
            out[m.group(1)] = (m.group(2) + "px", None)
    # 多行声明（--font-sans 跨两行）
    for m in re.finditer(r"(--font-(?:sans|mono))\s*:\s*([^;]+);", text, flags=re.S):
        out[m.group(1)] = (" ".join(m.group(2).split()), None)
    return out


def main() -> int:
    ref = CSS_REF
    if "--css" in sys.argv:
        ref = sys.argv[sys.argv.index("--css") + 1]
    css = read_css(ref)
    light = variables(block(css, ":root"))
    dark = variables(block(css, "html.dark"))
    dark_full = {**light, **dark}
    doc = doc_tokens(read_css(DOC_REF))

    print("# v1 设计令牌实测值（S01-06-8）\n")
    print(f"> 生成：`python3 scripts/v1-baseline/extract_tokens.py`，来源 `{ref}`（代码真值）。")
    print(f"> 对照：`{DOC_REF}`（已退役）。浅色 {len(light)} 个变量，深色覆盖 {len(dark)} 个。\n")

    print("## 与 design-tokens.md 的差异\n")
    diff_rows, missing_in_css = [], []
    for tok, (dv_light, dv_dark) in sorted(doc.items()):
        if tok not in light:
            missing_in_css.append(tok)
            continue
        cl = resolve(light[tok], light)
        cd = resolve(dark_full[tok], dark_full)
        if norm(resolve(dv_light, light)) != norm(cl):
            diff_rows.append((tok, "浅色/通用", dv_light, cl))
        if dv_dark is not None and norm(resolve(dv_dark, dark_full)) != norm(cd):
            diff_rows.append((tok, "深色", dv_dark, cd))
    print(f"### 值不一致（{len(diff_rows)}，`var()` 已展开后比较）\n")
    if diff_rows:
        print("| 令牌 | 主题 | design-tokens.md | global.css（真值） |\n|---|---|---|---|")
        for tok, theme, d, c in diff_rows:
            print(f"| `{tok}` | {theme} | `{d}` | `{c}` |")
    else:
        print("无")
    print(f"\n### 文档声明但代码不存在（{len(missing_in_css)}）\n")
    print(", ".join(f"`{t}`" for t in missing_in_css) or "无")
    only_css = sorted(set(light) - set(doc))
    print(f"\n### 代码存在但文档未声明（{len(only_css)}）\n")
    print(", ".join(f"`{t}`" for t in only_css) or "无")

    print("\n## 完整令牌表（global.css）\n")
    print("| 令牌 | 浅色（`:root`，展开后） | 深色（`html.dark`，展开后；空 = 同浅色） |\n|---|---|---|")
    for tok in sorted(light):
        lv = resolve(light[tok], light)
        dv = resolve(dark_full[tok], dark_full)
        print(f"| `{tok}` | `{lv}` | {f'`{dv}`' if dv != lv else ''} |")
    dark_only = sorted(set(dark) - set(light))
    if dark_only:
        print("\n深色独有：" + ", ".join(f"`{t}` = `{dark[t]}`" for t in dark_only))
    return 0


if __name__ == "__main__":
    sys.exit(main())
