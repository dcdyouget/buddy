#!/usr/bin/env python3
"""S03-02：把生成的 tokens.rs 与 WebKit 实算值逐个比对（独立真值，不复用生成器的解析）。

    python3 scripts/theme/verify_tokens.py

读取的是**已生成的产物** `crates/ui/src/theme_system/tokens.rs`，而非重新调用生成器 ——
验证对象就是要提交的东西。容差：RGB 各分量 ≤ 1/255，alpha ≤ 0.005（WebKit 序列化精度）。
"""

from __future__ import annotations

import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TOKENS = ROOT / "crates" / "ui" / "src" / "theme_system" / "tokens.rs"


def parse_tokens_rs() -> dict[str, dict[str, tuple]]:
    text = TOKENS.read_text(encoding="utf-8")
    struct = re.search(r"pub struct Palette \{(.*?)\n\}", text, flags=re.S).group(1)
    ident_to_name = dict(
        (m.group(2), m.group(1)) for m in re.finditer(r"/// `(--[a-z0-9-]+)`\n\s*pub ([a-z0-9_]+): Rgba", struct)
    )
    out = {}
    for mode, const in (("light", "LIGHT"), ("dark", "DARK")):
        body = re.search(rf"pub const {const}: Palette = Palette \{{(.*?)\n\}};", text, flags=re.S).group(1)
        out[mode] = {
            ident_to_name[m.group(1)]: tuple(float(m.group(i)) for i in range(2, 6))
            for m in re.finditer(r"([a-z0-9_]+): Rgba \{ r: ([\d.]+), g: ([\d.]+), b: ([\d.]+), a: ([\d.]+) \}", body)
        }
    return out


def main() -> int:
    ours = parse_tokens_rs()
    names = sorted(ours["light"])
    css = subprocess.run(
        ["git", "show", "v1-final:src/styles/global.css"], cwd=ROOT, check=True, capture_output=True, text=True
    ).stdout
    with tempfile.NamedTemporaryFile("w", suffix=".css", delete=False, encoding="utf-8") as f:
        f.write(css)
        css_path = f.name
    res = subprocess.run(
        ["swift", str(ROOT / "scripts" / "theme" / "verify_webkit.swift"), css_path, ",".join(names)],
        capture_output=True, text=True, timeout=180,
    )
    Path(css_path).unlink()
    if res.returncode != 0:
        print(res.stderr)
        return 2

    webkit = {"light": {}, "dark": {}}
    for line in res.stdout.splitlines():
        mode, name, *vals = line.split()
        webkit[mode][name] = tuple(float(v) for v in vals)

    bad, checked, worst = [], 0, 0.0
    for mode in ("light", "dark"):
        for name in names:
            a, b = ours[mode][name], webkit[mode].get(name)
            if b is None:
                bad.append(f"{mode} {name}: WebKit 无结果")
                continue
            checked += 1
            rgb_diff = max(abs(a[i] - b[i]) for i in range(3)) if b[3] > 0 else 0.0  # 全透明时 RGB 无意义
            a_diff = abs(a[3] - b[3])
            worst = max(worst, rgb_diff * 255)
            if rgb_diff > 1 / 255 + 1e-9 or a_diff > 0.005:
                bad.append(f"{mode} {name}: 生成 {a} vs WebKit {b}")
    print(f"比对 {checked} 个颜色（{len(names)} 令牌 × 浅/深），最大 RGB 偏差 {worst:.3f}/255，不一致 {len(bad)}")
    for b in bad:
        print("  " + b)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main())
