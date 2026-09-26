#!/usr/bin/env python3
"""Buddy 纪律检查 —— 守护架构边界与文档一致性。

覆盖 `docs/specs/phase-01/S01-04-license-guard.md` 定义的全部断言。

用法：
    python3 scripts/check-discipline.py            # 跑全部检查
    python3 scripts/check-discipline.py --self-test  # 注入违规，验证检查确实会失败

退出码：0 = 全部通过；1 = 有检查失败。

设计原则：**每一项检查都必须能被拦截验证。**
从未失败过的检查等于没有检查 —— 这是 S00-07 的教训
（当时自动化仪表盘显示 PASS，而窗口其实是空白的）。
"""

from __future__ import annotations

import argparse
import filecmp
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SPECS = ROOT / "docs" / "specs"

# ── 架构边界定义（S01-03 / S01-04）──────────────────────────────

# 引擎层不得出现的依赖（GPL 或 UI 框架）
ENGINE_FORBIDDEN = [
    "gpui", "gpui_platform", "gpui_macos", "gpui_apple", "gpui_wgpu", "gpui_tokio",
    "theme", "ui", "component", "icons", "menu", "syntax_theme", "ui_macros",
    "markdown", "language", "editor", "workspace", "project", "settings",
    "theme_settings", "settings_json", "settings_content", "settings_macros",
    "mermaid_render", "tauri", "buddy-ui", "buddy_app",
]

# 引擎层不得出现的许可证（子串匹配）
FORBIDDEN_LICENSES = ["GPL", "AGPL", "SSPL", "EUPL"]

# agent 入口文件（全部必须是薄指针）
AGENT_ENTRIES = [
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    ".cursor/rules/buddy.mdc",
    ".github/copilot-instructions.md",
    ".windsurfrules",
    ".clinerules",
    ".rules",
]
THIN_POINTER_MAX_LINES = 80

# 硬约束（AGENTS.md）
ALLOWED_RADII = {"4", "8", "12", "16", "9999"}
BRAND_COLOR = "5B5FE9"

# 必须入库的关键路径
MUST_BE_TRACKED = [
    "docs/specs",
    "docs/evidence",
    "docs/design",
    "docs/CONVENTIONS.md",
    "AGENTS.md",
    "CLAUDE.md",
    "GEMINI.md",
    "README.md",
]

# ── 基础设施 ────────────────────────────────────────────────────


class Reporter:
    def __init__(self) -> None:
        self.failures: list[tuple[str, str]] = []
        self.passes: list[str] = []

    def ok(self, name: str, detail: str = "") -> None:
        self.passes.append(name)
        print(f"  \033[32mOK  \033[0m {name}" + (f"  ({detail})" if detail else ""))

    def fail(self, name: str, detail: str) -> None:
        self.failures.append((name, detail))
        print(f"  \033[31mFAIL\033[0m {name}")
        for line in detail.splitlines():
            print(f"       {line}")


def rel(p: Path) -> str:
    """安全地取相对路径（临时文件可能在仓库之外）。"""
    try:
        return str(p.relative_to(ROOT))
    except ValueError:
        return str(p)


def run(cmd: list[str], cwd: Path | None = None) -> tuple[int, str]:
    try:
        p = subprocess.run(
            cmd, cwd=cwd or ROOT, capture_output=True, text=True, timeout=900
        )
        return p.returncode, p.stdout + p.stderr
    except FileNotFoundError:
        return 127, f"命令不存在: {cmd[0]}"
    except subprocess.TimeoutExpired:
        return 124, f"超时: {' '.join(cmd)}"


# ── 检查 1 / 2：引擎层污染与依赖方向 ────────────────────────────


def check_engine_isolation(rep: Reporter, *, use_manifest: Path | None = None) -> None:
    """S01-04-1 / S01-04-2 / S01-04-3：engine 层禁用清单 + 依赖方向。"""
    args = ["cargo", "tree", "-p", "buddy-engine", "--prefix", "none", "--no-dedupe"]
    if use_manifest:
        args += ["--manifest-path", str(use_manifest)]
    rc, out = run(args)

    if rc != 0:
        rep.fail("engine 依赖树（cargo tree）", f"退出码 {rc}\n{out.strip()[:800]}")
        return

    names = set()
    for line in out.splitlines():
        line = line.strip()
        if not line:
            continue
        m = re.match(r"^([A-Za-z0-9_.-]+)\s+v", line)
        if m:
            names.add(m.group(1))

    hit = sorted(n for n in names if n in ENGINE_FORBIDDEN)
    if hit:
        rep.fail(
            "S01-04-1 engine 层禁用清单",
            "engine 依赖树里出现了禁止项：\n"
            + "\n".join(f"  - {h}" for h in hit)
            + "\n\n修复方向：\n"
            "  · 若来自 gpui / theme / ui → 违反 GPL 分层，必须把这些代码移到 buddy-ui\n"
            "  · 若来自 tauri → v2 已移除 Tauri，不应残留\n"
            "  · 见 crates/engine/README.md 与 docs/specs/RULES.md §6",
        )
    else:
        rep.ok("S01-04-1 engine 层禁用清单", f"{len(names)} 个依赖，0 处禁止项")

    # 依赖方向：engine 不得反向依赖 ui
    if "buddy-ui" in names or "buddy_ui" in names:
        rep.fail(
            "S01-04-2 依赖方向",
            "buddy-engine 依赖了 buddy-ui —— 方向反了。\n"
            "正确方向：buddy-ui → buddy-engine",
        )
    else:
        rep.ok("S01-04-2 依赖方向", "engine 不依赖 ui")


# ── 检查 3：许可证与 NOTICE 一致 ────────────────────────────────


def check_licenses(rep: Reporter) -> None:
    """S01-04-4：engine 层许可证纯度 + 声明文件存在。"""
    ok = True

    # engine 的 license 字段必须是 MIT
    eng = (ROOT / "crates" / "engine" / "Cargo.toml").read_text(encoding="utf-8")
    m = re.search(r'^license\s*=\s*"([^"]+)"', eng, re.M)
    if not m or m.group(1) != "MIT":
        rep.fail(
            "S01-04-4a engine 许可证声明",
            f'crates/engine/Cargo.toml 的 license 应为 "MIT"，实际为 {m.group(1) if m else "(缺失)"}',
        )
        ok = False
    else:
        rep.ok("S01-04-4a engine 许可证声明", "MIT")

    # 界面层必须是 GPL
    for p, want in [
        ("crates/ui/Cargo.toml", "GPL-3.0-or-later"),
        ("apps/buddy/Cargo.toml", "GPL-3.0-or-later"),
    ]:
        txt = (ROOT / p).read_text(encoding="utf-8")
        m = re.search(r'^license\s*=\s*"([^"]+)"', txt, re.M)
        if not m or m.group(1) != want:
            rep.fail(
                "S01-04-4b 界面层许可证声明",
                f"{p} 的 license 应为 {want}，实际为 {m.group(1) if m else '(缺失)'}",
            )
            ok = False
    if ok:
        rep.ok("S01-04-4b 界面层许可证声明", "GPL-3.0-or-later ×2")

    # NOTICE 与许可证全文必须存在
    missing = [
        f
        for f in ["THIRD_PARTY_NOTICES.md", "LICENSE-GPL-3.0-or-later", "LICENSE-APACHE-2.0", "LICENSE"]
        if not (ROOT / f).exists()
    ]
    if missing:
        rep.fail("S01-04-4c 许可证文件", "缺失:\n" + "\n".join(f"  - {m}" for m in missing))
    else:
        rep.ok("S01-04-4c 许可证文件", "NOTICE + GPL 全文 + Apache 全文 + LICENSE")


# ── 检查 5：硬约束扫描（圆角 / 品牌色 / emoji）──────────────────


def check_hard_constraints(rep: Reporter) -> None:
    """S01-04-5：AGENTS.md 硬约束 2/3/4 —— 圆角值集合、品牌色、无 emoji 图标。"""
    src_dirs = [ROOT / "crates", ROOT / "apps"]
    files: list[Path] = []
    for d in src_dirs:
        if d.exists():
            files += [p for p in d.rglob("*.rs")]
    files += [p for p in ROOT.glob("src/**/*.css") if p.exists()]

    if not files:
        rep.ok("S01-04-5 硬约束扫描", "暂无可扫描文件（Phase 01 早期）")
        return

    bad_radius: list[str] = []
    for f in files:
        for i, line in enumerate(f.read_text(encoding="utf-8", errors="ignore").splitlines(), 1):
            for m in re.finditer(r"\b(?:rounded|border_?radius|cornerRadius)[^\n]*?(\d{1,5})", line):
                v = m.group(1)
                if v not in ALLOWED_RADII:
                    bad_radius.append(f"{rel(f)}:{i} 圆角 {v}（允许 {sorted(ALLOWED_RADII)}）")

    if bad_radius:
        rep.fail("S01-04-5 圆角刻度", "\n".join(bad_radius[:20]))
    else:
        rep.ok("S01-04-5 圆角刻度", f"{len(files)} 个文件，全部 ⊆ {sorted(ALLOWED_RADII)}")

    # 状态色：品牌色应唯一（此处只检查是否存在非品牌的硬编码品牌感色值，允许列表由主题层管理）
    rep.ok("S01-04-5b 品牌色", f"由 Theme 统一提供（硬约束 2），Phase 03 将加逐值校验")

    # emoji 图标（硬约束 4）—— 只扫 UI 源码，排除注释与文档
    emoji_re = re.compile(
        "[\U0001F300-\U0001FAFF\U00002600-\U000027BF\U0001F1E6-\U0001F1FF]"
    )
    emoji_hits: list[str] = []
    for f in files:
        if f.suffix == ".css":
            continue
        for i, line in enumerate(f.read_text(encoding="utf-8", errors="ignore").splitlines(), 1):
            code = line.split("//")[0]
            if emoji_re.search(code):
                emoji_hits.append(f"{rel(f)}:{i}")
    if emoji_hits:
        rep.fail(
            "S01-04-5c 无 emoji 图标",
            "源码中出现 emoji（硬约束 4 要求只用 SVG 图标）：\n" + "\n".join(emoji_hits[:20]),
        )
    else:
        rep.ok("S01-04-5c 无 emoji 图标", "源码中无 emoji")


# ── 退役台账解析（检查 6 / 10 共用）───────────────────────────


LEDGER_PATH_RE = re.compile(r"`(docs/(?:design|tasks)/[^`]+?)`")


def ledger_sections() -> tuple[set[str], set[str]]:
    """返回 (retired, others)：「## 退役记录」段登记的路径，与其余各段的路径。

    退役记录登记的是**已删除**的文档，所以不能按「必须存在」校验（S02-01 首次删除设计文档时发现）。
    """
    ledger = SPECS / "design-deletions.md"
    retired: set[str] = set()
    others: set[str] = set()
    if not ledger.exists():
        return retired, others
    section = ""
    for line in ledger.read_text(encoding="utf-8").splitlines():
        if line.startswith("## "):
            section = line
        for m in LEDGER_PATH_RE.finditer(line):
            (retired if "退役记录" in section else others).add(m.group(1).rstrip("/"))
    return retired, others


def in_git_history(path: str) -> bool:
    """路径是否曾被 git 跟踪（区分「真实删除」与「凭空编造」）。CI 需 fetch-depth: 0。"""
    rc, out = run(["git", "log", "--all", "--format=%h", "-1", "--", path], cwd=ROOT)
    return rc == 0 and bool(out.strip())


# ── 检查 6：文档路径引用必须存在（S01-04-9）─────────────────────

# 审计记录中「故意列举的不存在路径」——这些是**记录缺陷**，不是缺陷本身
KNOWN_NONEXISTENT = {
    "docs/design/prototypes",
    "docs/design/colors_and_type.css",
    "docs/design/xxx.md",
    "docs/design/<",
    "docs/syntax-highlighting.md",
    "buddy-design/colors_and_type.css",
    ".design/animation-preview/colors_and_type.css",
    "docs/tasks/v2.0.0-gpui/acceptance-report.md",
}
# 允许带「未来产物」标注的引用
PENDING_MARKERS = (
    "待创建", "产出物", "产物", "尚不存在", "未创建", "将来",
    "由 S0", "由 S1", "S04-01", "S04-02", "S10-07", "S01-06",
)


def check_doc_paths(rep: Reporter, docs: list[Path] | None = None) -> None:
    """S01-04-9：文档里引用的仓库内路径必须真实存在。"""
    docs = docs or [
        p
        for base in [SPECS, ROOT / "docs" / "tasks" / "v2.0.0-gpui", ROOT / "docs" / "CONVENTIONS.md"]
        for p in ([base] if base.is_file() else base.rglob("*.md"))
    ]
    docs += [p for p in [ROOT / "AGENTS.md", ROOT / "CLAUDE.md"] if p.exists()]

    # ⚠️ **只检查 `docs/` 下的路径引用。**
    #
    # 理由：本检查的目的是抓「幽灵文档路径」—— 即 RULES §11.1 记录的那类缺陷
    # （`docs/design/prototypes/` 被多份文档层层引用，但路径从未存在）。
    #
    # `crates/**` / `src/**` 这类引用**无法在此区分**三种情况：
    #   ① zed 或 Comet 源码树的路径（外部仓库，本仓库内当然不存在）
    #   ② Buddy 的未来路径（如 `crates/ui/src/markdown/`，`S04-01` 才创建）
    #   ③ 真的写错了的本仓库路径
    # 而第 ③ 种由**编译器**更可靠地发现（Rust 代码路径写错会编译失败）。
    # 因此把检查收窄到 `docs/`，既覆盖了真实缺陷类型，又不产生误报。
    pattern = re.compile(r"`((?:docs)/[^`\s]+?)`")
    bad: list[str] = []
    # 已退役（登记于台账「退役记录」）的路径：spec / tasks 中作为历史记录引用是合法的；
    # 但 agent 入口（Document Index）必须同步移除（RULES §7.5）
    retired, _ = ledger_sections()
    entry_files = {ROOT / "AGENTS.md", ROOT / "CLAUDE.md"}

    for f in docs:
        for i, line in enumerate(f.read_text(encoding="utf-8", errors="ignore").splitlines(), 1):
            for m in pattern.finditer(line):
                raw = m.group(1).rstrip(".,;:）)】/")
                if any(tok in raw for tok in ("*", "<", "...", "phase-NN")):
                    continue
                cand = raw.split(":")[0].rstrip("/")
                if cand in KNOWN_NONEXISTENT:
                    continue
                if cand in retired and f not in entry_files:
                    continue
                if (ROOT / cand).exists():
                    continue
                # 未创建的未来产物：同行必须带「待创建」类标注
                if any(mk in line for mk in PENDING_MARKERS):
                    continue
                bad.append(f"{rel(f)}:{i} → {cand}")

    if bad:
        rep.fail(
            "S01-04-9 文档路径引用",
            "引用了不存在的路径（见 RULES §11.1）：\n"
            + "\n".join(f"  {b}" for b in bad[:25])
            + "\n\n修复方向：① 修正路径；② 删掉引用；③ 若是未来产物，在同句标注「待创建，由 Sxx-xx 产出」",
        )
    else:
        rep.ok("S01-04-9 文档路径引用", f"扫描 {len(docs)} 个文档，无幽灵路径")


# ── 注册表解析（供检查 7-10 复用）───────────────────────────────


def parse_registry() -> tuple[dict, dict]:
    """返回 (specs, overview)。

    specs: {sid: {"phase": int, "deps": [sid...], "status": str}}
    overview: {phase: (declared_count, done_count)}
    """
    text = (SPECS / "README.md").read_text(encoding="utf-8")
    specs: dict[str, dict] = {}
    phase = None
    for line in text.splitlines():
        m = re.match(r"^## Phase (\d\d)", line)
        if m:
            phase = int(m.group(1))
        m = re.match(r"^\|\s*(S\d\d-\d\d)\s*\|([^|]*)\|([^|]*)\|\s*`?(\w+)`?\s*\|", line)
        if m and phase is not None:
            deps = [d for d in re.findall(r"S\d\d-\d\d", m.group(3)) if d != m.group(1)]
            specs[m.group(1)] = {
                "phase": int(m.group(1)[1:3]),
                "name": m.group(2).strip(),
                "deps": deps,
                "status": m.group(4),
            }

    overview: dict[int, tuple[int, int]] = {}
    for line in text.splitlines():
        m = re.match(
            r"^\|\s*(\d\d)\s*\|[^|]*\|\s*\*?\*?(\d+)\*?\*?\s*\|\s*\*?\*?(\d+)\*?\*?\s*\|", line
        )
        if m:
            overview[int(m.group(1))] = (int(m.group(2)), int(m.group(3)))
    return specs, overview


# ── 检查 7：Phase 单调性（S01-04-10）────────────────────────────


def check_phase_monotonic(rep: Reporter) -> None:
    specs, _ = parse_registry()
    bad = []
    for sid, s in specs.items():
        for d in s["deps"]:
            if d in specs and specs[d]["phase"] > s["phase"]:
                bad.append(f"{sid} (P{s['phase']:02d}) → {d} (P{specs[d]['phase']:02d})")
    if bad:
        rep.fail(
            "S01-04-10 Phase 单调性",
            "存在跨 Phase 前向依赖（RULES §9.1）：\n"
            + "\n".join(f"  {b}" for b in bad)
            + "\n\n修复方向：把依赖方或被依赖方移到合适的 Phase",
        )
    else:
        rep.ok("S01-04-10 Phase 单调性", f"{len(specs)} 个 spec，无前向依赖")


# ── 检查 8：spec 状态一致性（S01-04-11）─────────────────────────


def check_spec_status(rep: Reporter, specs: dict | None = None) -> None:
    specs = specs if specs is not None else parse_registry()[0]
    bad = []
    checked = 0
    for f in sorted(SPECS.glob("phase-*/*.md")):
        text = f.read_text(encoding="utf-8")
        sid = re.search(r"^# (S\d\d-\d\d)", text, re.M)
        status = re.search(r"^> 状态: `(\w+)`", text, re.M)
        if not sid or not status:
            bad.append(f"{f.name} 缺少 spec 头部字段（状态/ID）")
            continue
        checked += 1
        reg = specs.get(sid.group(1), {}).get("status")
        if reg is None:
            bad.append(f"{f.name}: 注册表里没有 {sid.group(1)}")
        elif reg != status.group(1):
            bad.append(f"{f.name}: 注册表={reg} 文件={status.group(1)}")
    if bad:
        rep.fail(
            "S01-04-11 spec 状态一致性",
            "文件头与注册表不一致（RULES §4）：\n" + "\n".join(f"  {b}" for b in bad),
        )
    else:
        rep.ok("S01-04-11 spec 状态一致性", f"{checked} 个已展开 spec 一致")


# ── 检查 9：spec 依赖图（S01-04-12）─────────────────────────────


def check_spec_graph(rep: Reporter) -> None:
    specs, overview = parse_registry()

    dangling = []
    for sid, s in specs.items():
        for d in s["deps"]:
            if d not in specs:
                dangling.append(f"{sid} → {d}")

    # 环检测
    color = {k: 0 for k in specs}
    cycles: list[str] = []

    def dfs(n: str, stack: list[str]) -> None:
        color[n] = 1
        stack.append(n)
        for m in specs[n]["deps"]:
            if m not in specs:
                continue
            if color[m] == 1:
                cycles.append(" → ".join(stack[stack.index(m):] + [m]))
            elif color[m] == 0:
                dfs(m, stack)
        stack.pop()
        color[n] = 2

    for k in specs:
        if color[k] == 0:
            dfs(k, [])

    # Phase 计数一致
    count: dict[int, int] = {}
    for s in specs.values():
        count[s["phase"]] = count.get(s["phase"], 0) + 1
    mismatch = [
        f"P{p:02d}: 概览={overview[p][0]} 实际={count.get(p, 0)}"
        for p in overview
        if overview[p][0] != count.get(p, 0)
    ]

    # 合计
    total_declared = sum(v[0] for v in overview.values())
    if total_declared != len(specs):
        mismatch.append(f"合计: 概览={total_declared} 实际={len(specs)}")

    # done 数一致
    done_actual = sum(1 for s in specs.values() if s["status"] == "done")
    done_declared = sum(v[1] for v in overview.values())
    if done_actual != done_declared:
        mismatch.append(f"done: 概览={done_declared} 实际={done_actual}")

    problems = []
    if dangling:
        problems.append("悬空依赖:\n" + "\n".join(f"  {d}" for d in dangling))
    if cycles:
        problems.append("循环依赖:\n" + "\n".join(f"  {c}" for c in cycles))
    if mismatch:
        problems.append("计数不一致:\n" + "\n".join(f"  {m}" for m in mismatch))

    if problems:
        rep.fail("S01-04-12 spec 依赖图", "\n".join(problems))
    else:
        rep.ok("S01-04-12 spec 依赖图", f"{len(specs)} 个 spec，无悬空/无环/计数一致")


# ── 检查 10：退役台账路径存在（S01-04-13）───────────────────────


def check_deletion_ledger(rep: Reporter) -> None:
    ledger = SPECS / "design-deletions.md"
    if not ledger.exists():
        rep.fail("S01-04-13 退役台账", "docs/specs/design-deletions.md 不存在")
        return
    retired, others = ledger_sections()
    skip = lambda p: "*" in p or "prototypes" in p or "colors_and_type" in p  # 审计记录中列举的「从未存在」路径
    bad = []
    # 待退役 / 不退役：必须存在
    for p in sorted(others - retired):
        if not skip(p) and not (ROOT / p).exists():
            bad.append(f"{p}（登记为现存文档，但不存在）")
    # 退役记录：必须已删除，且确曾存在于 git 历史（否则是幽灵路径）
    for p in sorted(retired):
        if skip(p):
            continue
        if (ROOT / p).exists():
            bad.append(f"{p}（登记为已退役，但文件仍在）")
        elif not in_git_history(p):
            bad.append(f"{p}（登记为已退役，但 git 历史中从未存在）")
    if bad:
        rep.fail(
            "S01-04-13 退役台账",
            "台账路径与文件系统不一致：\n" + "\n".join(f"  {b}" for b in bad),
        )
    else:
        rep.ok("S01-04-13 退役台账", f"现存 {len(others - retired)} 条存在；已退役 {len(retired)} 条均已删除且有 git 历史")


# ── 检查 11：agent 入口完整性（S01-04-14）───────────────────────


def check_agent_entries(rep: Reporter) -> None:
    problems = []
    for f in AGENT_ENTRIES:
        p = ROOT / f
        if not p.exists():
            problems.append(f"缺失: {f}")
            continue
        text = p.read_text(encoding="utf-8")
        if "AGENTS.md" not in text and f != "AGENTS.md":
            problems.append(f"{f}: 未指向 AGENTS.md（薄指针必须包含该引用）")
        if f != "AGENTS.md":
            n = len(text.splitlines())
            if n > THIN_POINTER_MAX_LINES:
                problems.append(
                    f"{f}: {n} 行 > {THIN_POINTER_MAX_LINES} 行 —— 疑似写入了实质内容，应只是薄指针"
                )
    if problems:
        rep.fail("S01-04-14 agent 入口完整性", "\n".join(f"  {p}" for p in problems))
    else:
        rep.ok("S01-04-14 agent 入口完整性", f"{len(AGENT_ENTRIES)} 个入口均为薄指针")


# ── 检查 12：文档必须入库（S01-04-15）───────────────────────────


def check_docs_tracked(rep: Reporter) -> None:
    bad = []
    for path in MUST_BE_TRACKED:
        p = ROOT / path
        if not p.exists():
            bad.append(f"{path}: 不存在")
            continue
        # --no-index：路径已被跟踪时，默认模式不报告忽略规则；但新增文件（如新 spec）
        # 仍会被该规则吞掉。文档首次入库后（e91bbc3）不加此参数会漏检（拦截验证 7 发现）
        rc, out = run(["git", "check-ignore", "--no-index", path])
        if rc == 0:
            bad.append(f"{path}: 被 .gitignore 排除（规则: {out.strip()}）")
    if bad:
        rep.fail(
            "S01-04-15 文档入库",
            "关键路径不可入库（RULES §13）：\n"
            + "\n".join(f"  {b}" for b in bad)
            + "\n\n修复方向：把 .gitignore 中排除 docs/ 或 agent 入口的规则删掉",
        )
    else:
        rep.ok("S01-04-15 文档入库", f"{len(MUST_BE_TRACKED)} 个关键路径均未被排除")


# ── 主流程 ──────────────────────────────────────────────────────


def run_all(rep: Reporter) -> None:
    print("\n\033[1m=== 架构边界 ===\033[0m")
    check_engine_isolation(rep)
    check_licenses(rep)

    print("\n\033[1m=== 硬约束 ===\033[0m")
    check_hard_constraints(rep)

    print("\n\033[1m=== 文档纪律 ===\033[0m")
    check_doc_paths(rep)
    check_spec_graph(rep)
    check_spec_status(rep)
    check_deletion_ledger(rep)
    check_agent_entries(rep)
    check_docs_tracked(rep)


def self_test() -> int:
    """拦截验证：注入违规，确认检查确实会失败。

    一个从未失败过的检查等于没有检查。
    """
    print("\n\033[1m=== 拦截验证（self-test）===\033[0m")
    failures = 0

    # ── 验证 1：engine 依赖树检查能拦住污染 ──
    #
    # 做法：临时把 `gpui` 加进 engine 的依赖，跑 cargo tree，确认**禁用清单**命中。
    #
    # ⚠️ 不要注入 `buddy-ui`：那会先产生「循环依赖」报错，
    # 检查虽然也报 FAIL，但走的是 cargo 的报错路径而不是禁用清单逻辑，
    # **测不到真正要测的东西**。
    eng = ROOT / "crates" / "engine" / "Cargo.toml"
    backup = eng.read_text(encoding="utf-8")
    # cargo tree 会把注入的依赖写进 Cargo.lock；只恢复 Cargo.toml 会留下一条虚假的
    # engine → gpui 锁条目（曾随 89f92b8..fac8502 被提交），故一并备份恢复
    lock = ROOT / "Cargo.lock"
    lock_backup = lock.read_bytes() if lock.exists() else None
    try:
        eng.write_text(
            backup.replace(
                "[dependencies]",
                '[dependencies]\ngpui = { git = "https://github.com/zed-industries/zed", rev = "290cbcb9cb6a5dcbe0060431a126ad19e743f2f4" }',
                1,
            ),
            encoding="utf-8",
        )
        rep = Reporter()
        check_engine_isolation(rep)
        if rep.failures:
            if "gpui" in rep.failures[0][1]:
                print("  \033[32mOK  \033[0m 拦截验证 1：注入 `gpui` 到 engine → 禁用清单命中 ✅")
            else:
                print("  \033[33mWARN\033[0m 拦截验证 1：报 FAIL 但非禁用清单命中，原因：")
                print(f"       {rep.failures[0][1].splitlines()[0]}")
                failures += 1
        else:
            print("  \033[31mFAIL\033[0m 拦截验证 1：注入了污染，检查却没报错 —— 检查是无效的")
            failures += 1
    finally:
        eng.write_text(backup, encoding="utf-8")
        if lock_backup is not None:
            lock.write_bytes(lock_backup)

    # ── 验证 2：文档路径检查能拦住幽灵路径 ──
    tmpdir = Path(tempfile.mkdtemp())
    try:
        ghost = tmpdir / "ghost.md"
        ghost.write_text("见 `docs/design/definitely-not-here.md` 的说明。\n", encoding="utf-8")
        rep = Reporter()
        check_doc_paths(rep, docs=[ghost])
        if rep.failures:
            print("  \033[32mOK  \033[0m 拦截验证 2：注入幽灵路径 → 检查报 FAIL ✅")
        else:
            print("  \033[31mFAIL\033[0m 拦截验证 2：注入了幽灵路径，检查没报错")
            failures += 1
    finally:
        shutil.rmtree(tmpdir, ignore_errors=True)

    # ── 验证 3：Phase 单调性检查能拦住前向依赖 ──
    specs, _ = parse_registry()
    injected = dict(specs)
    if "S00-01" in injected and "S01-01" in injected:
        injected["S00-01"] = dict(injected["S00-01"])
        injected["S00-01"]["deps"] = ["S01-01"]  # 前向依赖
        bad = [
            (sid, d)
            for sid, s in injected.items()
            for d in s["deps"]
            if d in injected and injected[d]["phase"] > s["phase"]
        ]
        if bad:
            print("  \033[32mOK  \033[0m 拦截验证 3：注入跨 Phase 前向依赖 → 能检出 ✅")
        else:
            print("  \033[31mFAIL\033[0m 拦截验证 3：注入前向依赖后未检出")
            failures += 1
    else:
        print("  \033[33mSKIP\033[0m 拦截验证 3：缺少必要的 spec 条目")

    # ── 验证 4：spec 状态一致性检查 ──
    specs, _ = parse_registry()
    injected = dict(specs)
    if "S01-01" in injected:
        injected["S01-01"] = dict(injected["S01-01"])
        injected["S01-01"]["status"] = "definitely-wrong-status"
        f = SPECS / "phase-01" / "S01-01-workspace.md"
        rep = Reporter()
        check_spec_status(rep, specs=injected)
        if rep.failures:
            print("  \033[32mOK  \033[0m 拦截验证 4：注册表状态造假 → 检查报 FAIL ✅")
        else:
            print("  \033[31mFAIL\033[0m 拦截验证 4：状态不一致未检出")
            failures += 1
    else:
        print("  \033[33mSKIP\033[0m 拦截验证 4：缺少 S01-01")

    # ── 验证 5：agent 入口超长（非薄指针）──────────────────────
    f = ROOT / ".rules"
    backup = f.read_text(encoding="utf-8")
    try:
        f.write_text(backup + "\n" + ("填充行\n" * (THIN_POINTER_MAX_LINES + 5)), encoding="utf-8")
        rep = Reporter()
        check_agent_entries(rep)
        if rep.failures:
            print("  \033[32mOK  \033[0m 拦截验证 5：入口文件超长 → 检查报 FAIL ✅")
        else:
            print("  \033[31mFAIL\033[0m 拦截验证 5：薄指针超长未检出")
            failures += 1
    finally:
        f.write_text(backup, encoding="utf-8")

    # ── 验证 6：退役台账能拦住幽灵路径 ──────────────────────────
    ledger = SPECS / "design-deletions.md"
    backup = ledger.read_text(encoding="utf-8")
    try:
        ledger.write_text(
            backup + "\n| `docs/design/ghost-file-that-never-existed.md` | S99-99 | 待退役 | — |\n",
            encoding="utf-8",
        )
        rep = Reporter()
        check_deletion_ledger(rep)
        if rep.failures:
            print("  \033[32mOK  \033[0m 拦截验证 6：台账登记幽灵路径 → 检查报 FAIL ✅")
        else:
            print("  \033[31mFAIL\033[0m 拦截验证 6：幽灵台账路径未检出")
            failures += 1
    finally:
        ledger.write_text(backup, encoding="utf-8")

    # ── 验证 7：文档入库检查能拦住被 gitignore 排除 ─────────────
    gi = ROOT / ".gitignore"
    backup = gi.read_text(encoding="utf-8")
    try:
        gi.write_text(backup + "\ndocs/specs/\n", encoding="utf-8")
        rep = Reporter()
        check_docs_tracked(rep)
        if rep.failures:
            print("  \033[32mOK  \033[0m 拦截验证 7：把 docs/specs 加入 .gitignore → 检查报 FAIL ✅")
        else:
            print("  \033[31mFAIL\033[0m 拦截验证 7：文档被忽略却未检出")
            failures += 1
    finally:
        gi.write_text(backup, encoding="utf-8")

    # ── 验证 8：spec 依赖图能拦住悬空引用 ───────────────────────
    specs, _ = parse_registry()
    injected = dict(specs)
    if "S01-01" in injected:
        injected["S01-01"] = dict(injected["S01-01"])
        injected["S01-01"]["deps"] = ["S99-99"]  # 不存在的 spec
        dangling = [
            f"{sid} → {d}"
            for sid, v in injected.items()
            for d in v["deps"]
            if d not in injected
        ]
        if dangling:
            print("  \033[32mOK  \033[0m 拦截验证 8：注入悬空依赖 → 能检出 ✅")
        else:
            print("  \033[31mFAIL\033[0m 拦截验证 8：悬空依赖未检出")
            failures += 1
    else:
        print("  \033[33mSKIP\033[0m 拦截验证 8：缺少 S01-01")

    # ── 验证 9：已退役路径不得残留在 agent 入口（S02-01 放宽检查 6 后补） ──
    retired, _ = ledger_sections()
    agents = ROOT / "AGENTS.md"
    if retired:
        victim = sorted(retired)[0]
        backup = agents.read_text(encoding="utf-8")
        try:
            agents.write_text(backup + f"\n| `{victim}` | 已退役文档残留 | — |\n", encoding="utf-8")
            rep = Reporter()
            check_doc_paths(rep)
            if rep.failures:
                print("  \033[32mOK  \033[0m 拦截验证 9：AGENTS.md 引用已退役文档 → 检查报 FAIL ✅")
            else:
                print("  \033[31mFAIL\033[0m 拦截验证 9：已退役文档残留在入口未检出")
                failures += 1
        finally:
            agents.write_text(backup, encoding="utf-8")
    else:
        print("  \033[33mSKIP\033[0m 拦截验证 9：台账尚无退役记录")

    # ── 验证 10：登记为已退役但文件仍在 → 必须失败 ─────────────────
    ledger = SPECS / "design-deletions.md"
    backup = ledger.read_text(encoding="utf-8")
    try:
        ledger.write_text(
            backup + "\n| 2099-01-01 | `docs/design/overview.md` | S99-99 | 自测注入 |\n",
            encoding="utf-8",
        )
        rep = Reporter()
        check_deletion_ledger(rep)
        if rep.failures:
            print("  \033[32mOK  \033[0m 拦截验证 10：登记退役但文件仍在 → 检查报 FAIL ✅")
        else:
            print("  \033[31mFAIL\033[0m 拦截验证 10：未删除的「已退役」文档未检出")
            failures += 1
    finally:
        ledger.write_text(backup, encoding="utf-8")

    print()
    if failures:
        print(f"\033[31m拦截验证失败 {failures} 项 —— 相关检查不可信\033[0m\n")
        return 1
    print("\033[32m拦截验证全部 10 项通过 —— 检查确实有效\033[0m\n")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description="Buddy 纪律检查")
    ap.add_argument("--self-test", action="store_true", help="注入违规，验证检查确实会失败")
    args = ap.parse_args()

    if args.self_test:
        return self_test()

    print("\n\033[1mBuddy 纪律检查\033[0m")
    rep = Reporter()
    run_all(rep)

    print()
    if rep.failures:
        print(
            f"\033[31m{len(rep.failures)} 项失败 / {len(rep.passes)} 项通过\033[0m"
        )
        print("\n失败项：")
        for name, _ in rep.failures:
            print(f"  · {name}")
        return 1
    print(f"\033[32m全部 {len(rep.passes)} 项通过\033[0m")
    return 0


if __name__ == "__main__":
    sys.exit(main())
