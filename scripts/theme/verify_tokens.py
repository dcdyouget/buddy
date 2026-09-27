#!/usr/bin/env python3
"""S03-02 / S03-03 / S03-04：把生成的 tokens.rs 与 WebKit 实算值逐个比对（独立真值，不复用生成器的解析）。

    python3 scripts/theme/verify_tokens.py

读取的是**已生成的产物** `crates/ui/src/theme_system/tokens.rs`，而非重新调用生成器 ——
验证对象就是要提交的东西。三类：
- 颜色：`color: var(--x)` 的计算值；容差 RGB ≤ 1/255、alpha ≤ 0.005
- 阴影 / 发光：`box-shadow` / `filter` 的计算值，逐层比对偏移、模糊、扩展、颜色、inset
- 常量（尺寸 / 字重 / 时长 / 缓动 / 字体栈）：WebKit 读出的自定义属性值，按单位换算后比对
"""

from __future__ import annotations

import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOKENS = ROOT / "crates" / "ui" / "src" / "theme_system" / "tokens.rs"
NUM = r"-?[\d.]+(?:e-?\d+)?"


# ───────────────────────────── WebKit ─────────────────────────────

def webkit(prop: str, names: list[str]) -> dict[str, dict[str, str]]:
    css = subprocess.run(
        ["git", "show", "v1-final:src/styles/global.css"], cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout
    with tempfile.NamedTemporaryFile("w", suffix=".css", delete=False, encoding="utf-8") as f:
        f.write(css)
        path = f.name
    try:
        res = subprocess.run(
            ["swift", str(ROOT / "scripts" / "theme" / "verify_webkit.swift"), path, prop, ",".join(names)],
            capture_output=True, text=True, timeout=240,
        )
    finally:
        Path(path).unlink()
    if res.returncode != 0:
        raise SystemExit(f"WebKit 失败：{res.stderr}")
    out: dict[str, dict[str, str]] = {"light": {}, "dark": {}}
    for line in res.stdout.splitlines():
        if not line.strip():
            continue
        mode, name, value = line.split("\t", 2)
        out[mode][name] = value
    return out


def parse_css_color(s: str) -> tuple:
    s = s.strip()
    nums = [float(x) for x in re.findall(NUM, s)]
    if s.startswith("color(srgb"):
        return (nums[0], nums[1], nums[2], nums[3] if len(nums) > 3 else 1.0)
    if s.startswith("rgb"):
        return (nums[0] / 255, nums[1] / 255, nums[2] / 255, nums[3] if len(nums) > 3 else 1.0)
    raise ValueError(s)


def split_top(s: str) -> list[str]:
    out, depth, cur = [], 0, ""
    for ch in s:
        depth += ch == "("
        depth -= ch == ")"
        if ch == "," and depth == 0:
            out.append(cur.strip())
            cur = ""
        else:
            cur += ch
    return [p for p in out + [cur.strip()] if p]


def parse_css_shadows(s: str) -> list[tuple]:
    """WebKit 计算值 → [(x, y, blur, spread, r, g, b, a, inset)]"""
    s = s.strip()
    if s.startswith("drop-shadow("):
        parts = re.findall(r"drop-shadow\(((?:[^()]|\([^()]*\))*)\)", s)
    else:
        parts = split_top(s)
    layers = []
    for p in parts:
        m = re.match(r"((?:rgba?|color)\([^)]*\))\s*(.*)", p.strip())
        color = parse_css_color(m.group(1))
        rest = m.group(2)
        inset = "inset" in rest
        lens = [float(x) for x in re.findall(r"(-?[\d.]+)px", rest)] + [0.0] * 4
        layers.append((*lens[:4], *color, inset))
    return layers


# ───────────────────────────── tokens.rs ─────────────────────────────

def ident_map(text: str, struct: str, ty: str) -> dict[str, str]:
    body = re.search(rf"pub struct {struct} \{{(.*?)\n\}}", text, flags=re.S).group(1)
    return {m.group(2): m.group(1) for m in re.finditer(rf"/// `(--[a-z0-9-]+)`\n\s*pub ([a-z0-9_]+): {re.escape(ty)}", body)}


def const_block(text: str, name: str) -> str:
    return re.search(rf"pub const {name}: \w+ = \w+ \{{(.*?)\n\}};", text, flags=re.S).group(1)


RGBA = rf"Rgba \{{ r: ({NUM}), g: ({NUM}), b: ({NUM}), a: ({NUM}) \}}"


def ours_colors(text: str) -> dict[str, dict[str, tuple]]:
    names = ident_map(text, "Palette", "Rgba")
    return {
        mode: {names[m.group(1)]: tuple(float(m.group(i)) for i in range(2, 6))
               for m in re.finditer(rf"([a-z0-9_]+): {RGBA}", const_block(text, c))}
        for mode, c in (("light", "LIGHT"), ("dark", "DARK"))
    }


def ours_shadows(text: str) -> dict[str, dict[str, list]]:
    names = ident_map(text, "Shadows", "&'static [ShadowSpec]")
    out = {}
    for mode, c in (("light", "LIGHT_SHADOWS"), ("dark", "DARK_SHADOWS")):
        body = const_block(text, c)
        out[mode] = {}
        for m in re.finditer(r"([a-z0-9_]+): &\[\n(.*?)\n    \],", body, flags=re.S):
            layers = []
            for L in re.finditer(
                rf"x: ({NUM}), y: ({NUM}), blur: ({NUM}), spread: ({NUM}), color: {RGBA}, inset: (true|false)", m.group(2)
            ):
                layers.append((*(float(L.group(i)) for i in range(1, 9)), L.group(9) == "true"))
            out[mode][names[m.group(1)]] = layers
    return out


def ours_consts(text: str) -> dict[str, tuple[str, str]]:
    """{--name: (类型, 值字面量)}，从 metrics / motion / fonts 模块的注释 + 常量读取"""
    out = {}
    for m in re.finditer(r"/// `(--[a-z0-9-]+): [^`]*`\n\s*pub const [A-Z0-9_]+: ([^=]+) = (.*);", text):
        out[m.group(1)] = (m.group(2).strip(), m.group(3).strip())
    return out


# ───────────────────────────── 比对 ─────────────────────────────

def close(a: float, b: float, tol: float) -> bool:
    return abs(a - b) <= tol + 1e-9


def check_colors(text: str, bad: list) -> int:
    ours = ours_colors(text)
    wk = webkit("color", sorted(ours["light"]))
    n = 0
    for mode in ("light", "dark"):
        for name, a in ours[mode].items():
            b = parse_css_color(wk[mode][name])
            n += 1
            rgb_ok = b[3] == 0 or all(close(a[i], b[i], 1 / 255) for i in range(3))
            if not (rgb_ok and close(a[3], b[3], 0.005)):
                bad.append(f"颜色 {mode} {name}: 生成 {a} vs WebKit {b}")
    return n


def check_shadows(text: str, bad: list) -> int:
    ours = ours_shadows(text)
    names = sorted(ours["light"])
    box = [n for n in names if n.startswith("--shadow-")]
    flt = [n for n in names if n.startswith("--filter-")]
    wk = {"light": {}, "dark": {}}
    for prop, group in (("box-shadow", box), ("filter", flt)):
        if group:
            r = webkit(prop, group)
            for mode in wk:
                wk[mode].update(r[mode])
    n = 0
    for mode in ("light", "dark"):
        for name, layers in ours[mode].items():
            theirs = parse_css_shadows(wk[mode][name])
            n += 1
            if len(layers) != len(theirs):
                bad.append(f"阴影 {mode} {name}: 层数 {len(layers)} vs {len(theirs)}")
                continue
            for i, (a, b) in enumerate(zip(layers, theirs)):
                geo = all(close(a[k], b[k], 0.01) for k in range(4))
                col = (b[7] == 0 or all(close(a[k], b[k], 1 / 255) for k in range(4, 7))) and close(a[7], b[7], 0.005)
                if not (geo and col and a[8] == b[8]):
                    bad.append(f"阴影 {mode} {name} 第 {i + 1} 层: 生成 {a} vs WebKit {b}")
    return n


def check_consts(text: str, bad: list) -> int:
    ours = ours_consts(text)
    wk = webkit("--raw", sorted(ours))["light"]
    n = 0
    for name, (ty, lit) in ours.items():
        raw = " ".join(wk[name].split())
        n += 1
        if ty == "&[&str]":
            a = re.findall(r'"([^"]*)"', lit)
            b = [f.strip().strip("'\"") for f in raw.split(",")]
        elif ty == "[f32; 4]":
            a = [float(x) for x in re.findall(NUM, lit)]
            b = [float(x) for x in re.findall(NUM, raw)]
        elif ty == "i32":
            a = [float(lit)]
            v = float(re.findall(NUM, raw)[0])
            b = [v * 1000 if raw.endswith("s") and not raw.endswith("ms") else v]
        else:  # f32：px / em / 纯数
            a = [float(lit)]
            b = [float(re.findall(NUM, raw)[0])]
        if len(a) != len(b) or any((x != y if isinstance(x, str) else not close(x, y, 1e-6)) for x, y in zip(a, b)):
            bad.append(f"常量 {name}: 生成 {lit} vs WebKit `{raw}`")
    return n


def main() -> int:
    text = TOKENS.read_text(encoding="utf-8")
    bad: list[str] = []
    nc = check_colors(text, bad)
    ns = check_shadows(text, bad)
    nk = check_consts(text, bad)
    print(f"颜色 {nc} 项、阴影 {ns} 项（浅/深）、常量 {nk} 项与 WebKit 比对：不一致 {len(bad)}")
    for b in bad:
        print("  " + b)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
