#!/usr/bin/env python3
"""S03-02 / S03-03 / S03-04：由 v1 代码真值生成 Rust 设计令牌。

    python3 scripts/theme/gen_tokens.py          # 写 crates/ui/src/theme_system/tokens.rs
    python3 scripts/theme/gen_tokens.py --check  # 只比较，不一致则退出码 1（S03-07 / CI）

来源：`v1-final:src/styles/global.css`（`:root` = 浅色，`html.dark` = 深色覆盖）。
**为什么生成而非手抄**：139 个变量、大量嵌套 `color-mix`，手抄必然出错；生成器同时是完备性校验。

颜色语义：CSS Color 5 `color-mix(in srgb, A p%, B q%)` —— 预乘 alpha 后按百分比插值再反预乘；
只给一个百分比时另一个取补数；两者和 < 100% 时结果 alpha 乘以 和/100。`transparent` = rgba(0,0,0,0)。
"""

from __future__ import annotations

import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CSS_REF = "v1-final:src/styles/global.css"
OUT = ROOT / "crates" / "ui" / "src" / "theme_system" / "tokens.rs"

# 不迁移的令牌（必须给出理由；完备性校验把它们计为「已处置」）
EXCLUDED = {
    "--glass-outline": "S00-04 用户实机否决（白色描边观感为「四边白光」），v2 不迁移",
}
ALLOWED_RADII = {4.0, 8.0, 12.0, 16.0, 9999.0}  # 硬约束 3
BRAND = "#5B5FE9"  # 硬约束 2

sys.path.insert(0, str(ROOT / "scripts" / "v1-baseline"))
from extract_tokens import block, resolve, variables  # noqa: E402  复用同一解析，避免两份实现漂移


# ───────────────────────────── 基础解析 ─────────────────────────────

def split_top(s: str, sep: str) -> list[str]:
    """按 sep 切分，忽略括号内的 sep。"""
    out, depth, cur = [], 0, ""
    for ch in s:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        if ch == sep and depth == 0:
            out.append(cur)
            cur = ""
        else:
            cur += ch
    out.append(cur)
    return [p.strip() for p in out if p.strip()]


def split_ws(s: str) -> list[str]:
    """按空白切分，忽略括号内的空白。"""
    out, depth, cur = [], 0, ""
    for ch in s:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
        if ch.isspace() and depth == 0:
            if cur:
                out.append(cur)
            cur = ""
        else:
            cur += ch
    if cur:
        out.append(cur)
    return out


Color = tuple  # (r, g, b, a) 0..1


def parse_color(s: str) -> Color:
    s = " ".join(s.split())
    low = s.lower()
    if low == "transparent":
        return (0.0, 0.0, 0.0, 0.0)
    if s.startswith("#"):
        h = s[1:]
        if len(h) == 3:
            h = "".join(c * 2 for c in h) + "ff"
        elif len(h) == 6:
            h += "ff"
        if len(h) != 8:
            raise ValueError(f"无法解析颜色 {s}")
        return tuple(int(h[i : i + 2], 16) / 255 for i in range(0, 8, 2))
    m = re.fullmatch(r"rgba?\(([^)]*)\)", low)
    if m:
        parts = [p.strip() for p in m.group(1).split(",")]
        r, g, b = (float(p) / 255 for p in parts[:3])
        a = float(parts[3]) if len(parts) == 4 else 1.0
        return (r, g, b, a)
    m = re.fullmatch(r"color-mix\((.*)\)", s, flags=re.S)
    if m:
        args = split_top(m.group(1), ",")
        if args[0].replace(" ", "") != "insrgb":
            raise ValueError(f"只支持 in srgb：{s}")
        (c1, p1), (c2, p2) = (mix_arg(a) for a in args[1:3])
        return color_mix(c1, p1, c2, p2)
    raise ValueError(f"无法解析颜色 {s}")


def mix_arg(arg: str):
    parts = split_ws(arg)
    pct = None
    if parts[-1].endswith("%"):
        pct = float(parts[-1][:-1]) / 100
        parts = parts[:-1]
    return parse_color(" ".join(parts)), pct


def color_mix(c1: Color, p1, c2: Color, p2) -> Color:
    if p1 is None and p2 is None:
        p1 = p2 = 0.5
    elif p1 is None:
        p1 = 1 - p2
    elif p2 is None:
        p2 = 1 - p1
    total = p1 + p2
    alpha_mult = min(total, 1.0)
    p1, p2 = p1 / total, p2 / total
    a = c1[3] * p1 + c2[3] * p2
    if a == 0:
        return (0.0, 0.0, 0.0, 0.0)
    rgb = tuple((c1[i] * c1[3] * p1 + c2[i] * c2[3] * p2) / a for i in range(3))
    return (*rgb, a * alpha_mult)


def is_color(v: str) -> bool:
    try:
        parse_color(v)
        return True
    except (ValueError, IndexError):
        return False


def px(v: str) -> float:
    v = v.strip()
    if v == "0":
        return 0.0
    m = re.fullmatch(r"(-?[\d.]+)px", v)
    if not m:
        raise ValueError(f"期望 px：{v}")
    return float(m.group(1))


def parse_shadow_layers(v: str) -> list[dict]:
    """`box-shadow` 列表或 `drop-shadow(...)` 列表 → 层列表。"""
    layers = []
    if v.startswith("drop-shadow("):
        items = [m for m in split_ws(v)]
        for it in items:
            inner = re.fullmatch(r"drop-shadow\((.*)\)", it, flags=re.S).group(1)
            layers.append(layer(split_ws(inner)))
        return layers
    for part in split_top(v, ","):
        layers.append(layer(split_ws(part)))
    return layers


def layer(tokens: list[str]) -> dict:
    inset = "inset" in tokens
    tokens = [t for t in tokens if t != "inset"]
    lengths = [t for t in tokens if re.fullmatch(r"-?[\d.]+(px)?", t)]
    colors = [t for t in tokens if t not in lengths]
    if len(colors) != 1 or not 2 <= len(lengths) <= 4:
        raise ValueError(f"无法解析阴影层 {tokens}")
    nums = [px(l) for l in lengths] + [0.0] * (4 - len(lengths))
    return {"x": nums[0], "y": nums[1], "blur": nums[2], "spread": nums[3], "color": parse_color(colors[0]), "inset": inset}


def ms(v: str) -> int:
    m = re.fullmatch(r"(-?[\d.]+)(ms|s)", v.strip())
    if not m:
        raise ValueError(f"期望时长：{v}")
    n = float(m.group(1))
    return round(n * 1000 if m.group(2) == "s" else n)


def font_stack(v: str) -> list[str]:
    return [f.strip().strip("'\"") for f in v.split(",")]


# ───────────────────────────── 分类 ─────────────────────────────

def classify(name: str, value: str) -> str:
    if name.startswith(("--shadow-", "--filter-")):
        return "shadow"
    if name.startswith("--font-") and not name.startswith(("--font-size", "--font-weight")):
        return "font"
    if name.startswith(("--duration-", "--delay-")):
        return "duration"
    if name.startswith("--ease-"):
        return "easing"
    if name.startswith(("--space-", "--radius-", "--font-size-")):
        return "px"
    if name.startswith("--letter-spacing-"):
        return "em"
    if name.startswith(("--line-height-", "--font-weight-")):
        return "number"
    if is_color(value):
        return "color"
    raise ValueError(f"未分类的令牌 {name}: {value}")


def ident(name: str) -> str:
    return name[2:].replace("-", "_")


def rgba_lit(c: Color) -> str:
    return "Rgba { r: %.6f, g: %.6f, b: %.6f, a: %.6f }" % c


def hex_of(c: Color) -> str:
    return "#" + "".join("%02X" % round(x * 255) for x in c)


def f32(x: float) -> str:
    s = repr(float(x))
    return s if "." in s or "e" in s else s + ".0"


# ───────────────────────────── 生成 ─────────────────────────────

def generate() -> str:
    css = subprocess.run(["git", "show", CSS_REF], cwd=ROOT, check=True, capture_output=True, text=True).stdout
    light_raw = variables(block(css, ":root"))
    dark_raw = variables(block(css, "html.dark"))
    dark_full = {**light_raw, **dark_raw}
    names = sorted(light_raw)
    assert set(dark_raw) <= set(light_raw), f"深色独有令牌：{set(dark_raw) - set(light_raw)}"

    light = {n: resolve(light_raw[n], light_raw) for n in names}
    dark = {n: resolve(dark_full[n], dark_full) for n in names}

    groups: dict[str, list[str]] = {}
    for n in names:
        if n in EXCLUDED:
            continue
        groups.setdefault(classify(n, light[n]), []).append(n)

    # 非主题令牌不得在深色中被覆盖（否则会静默丢值）
    for kind in ("font", "duration", "easing", "px", "em", "number"):
        for n in groups.get(kind, []):
            if light[n] != dark[n]:
                raise ValueError(f"{n} 在深色中被覆盖，但分类 {kind} 为主题无关")

    # 硬约束自检
    for n in groups.get("px", []):
        if n.startswith("--radius-") and px(light[n]) not in ALLOWED_RADII:
            raise ValueError(f"圆角 {n}={light[n]} 违反硬约束 3")
    brand_like = [n for n in groups["color"] if n.startswith("--buddy-primary") and n.endswith(("primary", "-500"))]
    for n in brand_like:
        if hex_of(parse_color(light[n]))[:7] != BRAND:
            raise ValueError(f"{n} 不是品牌色 {BRAND}")

    o = []
    w = o.append
    w("//! 设计令牌 —— **自动生成，勿手改**。")
    w("//!")
    w("//! 生成：`python3 scripts/theme/gen_tokens.py`；校验：`--check`（纪律检查 S03-07 调用）。")
    w(f"//! 来源：`{CSS_REF}`（`:root` = 浅色，`html.dark` = 深色覆盖），{len(names)} 个变量，")
    w(f"//! 其中 {len(EXCLUDED)} 个按理由排除（见 [`EXCLUDED`]）。每个颜色后的注释为 `#RRGGBBAA`，便于人工复核。")
    w("")
    w("#![allow(missing_docs)] // 字段即 CSS 变量名，逐个写文档无信息量")
    w("")
    w("use super::ShadowSpec;")
    w("use gpui::Rgba;")
    w("")
    w(f"/// 源 CSS 变量总数（完备性校验：生成项 + 排除项 = 此数）")
    w(f"pub const SOURCE_TOKEN_COUNT: usize = {len(names)};")
    w("")
    w("/// 不迁移的令牌及理由")
    w("pub const EXCLUDED: &[(&str, &str)] = &[")
    for n, why in EXCLUDED.items():
        w(f'    ("{n}", "{why}"),')
    w("];")
    w("")

    # Palette
    w("/// 随主题变化的颜色（浅 / 深各一份）")
    w("#[derive(Clone, Copy, Debug, PartialEq)]")
    w("pub struct Palette {")
    for n in groups["color"]:
        w(f"    /// `{n}`")
        w(f"    pub {ident(n)}: Rgba,")
    w("}")
    for const, table in (("LIGHT", light), ("DARK", dark)):
        w("")
        w(f"pub const {const}: Palette = Palette {{")
        for n in groups["color"]:
            c = parse_color(table[n])
            w(f"    {ident(n)}: {rgba_lit(c)}, // {hex_of(c)}")
        w("};")

    # Shadows
    w("")
    w("/// 阴影与发光（`box-shadow` / `filter: drop-shadow` 的逐层数据）")
    w("#[derive(Clone, Copy, Debug)]")
    w("pub struct Shadows {")
    for n in groups["shadow"]:
        w(f"    /// `{n}`")
        w(f"    pub {ident(n)}: &'static [ShadowSpec],")
    w("}")
    for const, table in (("LIGHT_SHADOWS", light), ("DARK_SHADOWS", dark)):
        w("")
        w(f"pub const {const}: Shadows = Shadows {{")
        for n in groups["shadow"]:
            w(f"    {ident(n)}: &[")
            for L in parse_shadow_layers(table[n]):
                w(
                    f"        ShadowSpec {{ x: {f32(L['x'])}, y: {f32(L['y'])}, blur: {f32(L['blur'])}, "
                    f"spread: {f32(L['spread'])}, color: {rgba_lit(L['color'])}, inset: {str(L['inset']).lower()} }}, // {hex_of(L['color'])}"
                )
            w("    ],")
        w("};")

    # 常量模块
    def const_block(mod: str, doc: str, kinds: list[str], render):
        w("")
        w(f"/// {doc}")
        w(f"pub mod {mod} {{")
        for kind in kinds:
            for n in groups.get(kind, []):
                ty, val = render(kind, light[n])
                w(f"    /// `{n}: {light[n]}`")
                w(f"    pub const {ident(n).upper()}: {ty} = {val};")
        w("}")

    def metric(kind, v):
        if kind == "px":
            return "f32", f32(px(v))
        if kind == "em":
            return "f32", f32(0.0 if v.strip() == "0" else float(v.strip()[:-2]))
        return "f32", f32(float(v))

    const_block("metrics", "尺寸：间距 / 圆角 / 字号为逻辑像素；字距为 em 系数；行高为字号倍数；字重为 CSS 数值", ["px", "em", "number"], metric)

    def motion(kind, v):
        if kind == "duration":
            return "i32", str(ms(v))
        nums = re.fullmatch(r"cubic-bezier\(([^)]*)\)", v).group(1).split(",")
        return "[f32; 4]", "[" + ", ".join(f32(float(x)) for x in nums) + "]"

    const_block("motion", "动效：时长 / 延迟为毫秒（延迟可为负）；缓动为 cubic-bezier 四参数", ["duration", "easing"], motion)

    def fonts(kind, v):
        return "&[&str]", "&[" + ", ".join(f'"{f}"' for f in font_stack(v)) + "]"

    const_block("fonts", "字体栈（原样保留 CSS 顺序，含通用族名；平台映射见 S03-05）", ["font"], fonts)

    generated = sum(len(v) for v in groups.values())
    assert generated + len(EXCLUDED) == len(names), (generated, len(EXCLUDED), len(names))
    return "\n".join(o) + "\n"


def main() -> int:
    text = generate()
    if "--check" in sys.argv:
        if not OUT.exists() or OUT.read_text(encoding="utf-8") != text:
            print(f"{OUT.relative_to(ROOT)} 与 {CSS_REF} 不一致：请运行 python3 scripts/theme/gen_tokens.py")
            return 1
        print(f"{OUT.relative_to(ROOT)} 与 {CSS_REF} 一致")
        return 0
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text, encoding="utf-8")
    print(f"写入 {OUT.relative_to(ROOT)}（{len(text.splitlines())} 行）")
    return 0


if __name__ == "__main__":
    sys.exit(main())
