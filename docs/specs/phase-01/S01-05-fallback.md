# S01-05 迁移期目录与退路分支

> 状态: `done`
> Phase: 01
> 依赖: S01-01
> 阻塞: —
> 退役设计文档: —

## 目标

保证迁移期间可随时回退到 Tauri 版本，且两套代码并存不互相污染。

**产出物**：受保护的退路分支/tag + 迁移期目录约定 + 旧代码的处置时点。

## 输入

- `docs/tasks/v2.0.0-gpui/overview.md` —— 推进规则（Phase 09 前不删退路）
- `docs/tasks/v2.0.0-gpui/research-log.md` 风险 R8（无法回退）
- 现有仓库结构（`src/` / `src-tauri/` / `package.json`）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S01-05-1 | 退路 tag | 在迁移开始前，给当前可用的 Tauri 版本打 tag |
| S01-05-2 | 退路分支 | 建立受保护的退路分支；**在 macOS 全链路验收通过前不允许删除** |
| S01-05-3 | 目录并存约定 | 新代码放 `crates/` + `apps/`；旧代码暂留 `src/` + `src-tauri/`，两套并存至 Phase 05 |
| S01-05-4 | 构建隔离 | 确认新 workspace 不会意外编译旧代码，反之亦然 |
| S01-05-5 | 数据兼容验证 | 两套实现指向同一数据目录（为 S02-04 做准备） |
| S01-05-6 | 旧代码处置时点 | 明确：Phase 05 完成后开始清理，Phase 10 前清空 |
| S01-05-7 | 发布隔离 | 确认迁移期旧版本仍可正常发布与自动更新 |
| S01-05-8 | 回退演练 | 实际演练一次从退路分支构建并运行 |

## 验收标准

- [x] 退路 tag 已创建
- [x] 退路分支已创建 —— 本地 `v1-fallback`；**推送与 GitHub 分支保护按用户决策暂缓**（2026-09-26：「先 commit，不 push」），推送时补做保护
- [x] 新 workspace 与旧工程构建互不干扰
- [x] 回退演练成功：从退路分支能构建出可运行的旧版本 —— 构建与测试通过；**用户实机运行确认**
- [x] 迁移期旧版本仍可发布与自动更新（静态核对；未实际发布，见证据）
- [x] 旧代码处置时点已书面明确
- [x] 数据目录一致性已确认（同一路径）

## 证据

| 项 | 证据 |
|----|------|
| tag 与分支 | v1 改动单独提交为 `9cc244a`（仅 `src/` `src-tauri/` `README.md`，37 文件 +2535/-459；暂存后 `git diff --cached --name-only \| grep -vE '^(src/\|src-tauri/\|README\.md$)'` 输出为空，证明无 v2 产物混入）。annotated tag `v1-final` → `9cc244a`（tag 对象 `88d9e10`）；分支 `v1-fallback` → `9cc244a` |
| 回退演练 | `git worktree add --detach /tmp/buddy-v1-drill v1-final`：`npx tsc --noEmit` rc=0；`npx vite build` ✓；`npx vitest run` **24 文件 / 99 测试全过**；`src-tauri cargo build` rc=0（`target/debug/buddy` 63 MB）；`cargo test` **166 passed / 0 failed / 8 ignored** |
| 构建隔离验证 | tag 工作树根目录无 `Cargo.toml` / `crates/`（`test ! -e` 通过）→ v1 不会被 v2 workspace 吸入。main 上反向：根 `Cargo.toml` `exclude = ["src-tauri"]`，`cd src-tauri && cargo check` rc=0；`cargo check --workspace` rc=0 且 engine 依赖树 0 处 tauri（`check-discipline.py` S01-04-1） |
| 发布隔离 | `scripts/release-macos.sh:137,172` 与 `scripts/set-version.mjs:35` 只引用 `src-tauri/Cargo.toml`，不读根 workspace；updater `endpoints` 在 `src-tauri/tauri.conf.json:70`，tag 上未改。**未执行真实发布**（会对外发布，不在演练范围） |
| 用户目检 | 2026-09-26 用户在 tag 工作树 `npm run tauri dev` 运行后反馈：「看到的效果和现在的差不多，应该是可以的」。演练工作树随后按用户要求删除（`git worktree remove`），tag / 分支保留 |
| 数据目录路径 | v1：`src-tauri/tauri.conf.json:5` `identifier = com.buddy.chat` → `~/Library/Application Support/com.buddy.chat`（S00-08 实测 v2 引擎直接读取该目录的 `config.json`，research-log §17.8）。v2 沿用同一路径由 `S02-04` 落实 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 退路基点 | 先单独提交用户未提交的 v1 改动（`9cc244a`），再在其上打 tag（2026-09-26 用户选 a） | HEAD `425bfc7` 不含 +1461 行在用 v1 功能，直接 tag 会让退路落后于实际使用的版本；且视觉验收已改为「对照运行中的 v1 目检」（见 S01-06 决策记录），退路必须就是当前 v1 |
| tag 与分支并存 | tag `v1-final` 不可移动；分支 `v1-fallback` 用于必要的 v1 热修 | tag 保证基准不漂移；热修走分支，不污染 main 的迁移历史 |
| 旧代码处置时点 | Phase 05 完成后开始清理 `src/` `src-tauri/`；Phase 10 前清空 | Phase 05 前 v1 是唯一可用版本与视觉对照基准 |
| 退路保留时长 | tag 与分支**永久保留**；分支保护至少到 v2 首次正式发布之后 | 见备注：发布 GPL 版本后无法回到闭源路线；保留成本为零 |
| 历史数据迁移 | **不做**（2026-09-26 用户决策：仍在开发阶段，不考虑历史数据迁移） | v2 沿用同一数据目录是零成本的附带结果，不为此写迁移/回归测试 |

## 完成记录

- 日期：2026-09-26
- commit：v1 基点 `9cc244a`（tag `v1-final` / 分支 `v1-fallback`，**仅本地**）；Phase 产物 `e91bbc3`
- 设计文档处置：—

**未完成的对外步骤**：`git push origin main v1-final v1-fallback` + GitHub 对 `v1-fallback` 加分支保护 —— 待用户决定推送时执行

## 备注

**GPL 影响此处的处置方式**：一旦迁移完成并发布 GPL 版本，无法再回到闭源或 Mac App Store 路线。因此退路分支的保留时长应覆盖到首次正式发布之后，而非仅到 macOS 验收通过。
