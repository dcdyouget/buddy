# S07-05 置顶与全工作区可见

> 状态: `doing`
> Phase: 07
> 依赖: S07-02
> 阻塞: —
> 退役设计文档: —

## 目标

依当前 v1 普通 level 0 + 前置 / 激活，不永久置顶。

## 输入

- `docs/tasks/v2.0.0-gpui/07-shell.md`
- `src-tauri/src/platform/macos.rs`
- `src-tauri/src/platform/windows.rs`
- `src-tauri/tauri.conf.json`

## 实现要点

- 依当前 v1 普通 level 0 + 前置 / 激活，不永久置顶。
- 探测 AllSpaces / FullScreenAuxiliary，协调独立模型菜单；Windows 实测留 Phase 09。

## 验收标准

- [ ] 唤起前置但不长期压住其他应用；跨桌面 / 全屏空间有实际证据。
- [ ] 实际原生层级和工作区读回，关键分支有效拦截。
- [ ] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

实现与接线已完成，尚未通过真实系统验收：

- `apps/buddy/examples/shell_preview/fullscreen_target.rs` 启动独立 GPUI child，使用普通
  `WindowKind::Normal`，通过 `Window::toggle_fullscreen()` 进入真实全屏 Space。只有
  `is_fullscreen=true`、`is_window_active=true` 、真实 `isOnActiveSpace=true` 和 bounds 覆盖整屏同时成立才写 READY；
  child 重新激活写 ACK，退出前先切回 windowed 并写 DONE。
- `crates/ui/src/shell/workspaces.rs` 通过 GPUI raw window handle 和 AppKit `objc_msgSend`
  实际读回 `NSWindow.windowNumber`、`isOnActiveSpace`、fullscreen style mask 与 frame；
  `apps/buddy/examples/shell_preview/level_native.rs` 再通过 CoreGraphics
  `CGWindowListCopyWindowInfo` 保存 on-screen window stack 的窗口号、层级和 bounds 相交关系。
  主窗口验收值为 `level=0`、`collection_behavior=0x101`，覆盖判定必须命中 child 的真实
  window number 及前台栈相交窗口。
- `apps/buddy/examples/shell_preview/t50_level.rs` 复用 Runtime 中已有主窗口和已安装的
  唯一热键，顺序执行：child READY → 热键唤起 Buddy → 主窗口原生读回 → child ACK 再次
  激活 → 主窗口失活且 child 仍全屏 → RAII 退出清理。不会再次 install runtime。
- 可选真实渲染证据：设置 `BUDDY_SHELL_T50_CAPTURE_DIR` 后，测试在
  `fullscreen-child`、`buddy-in-fullscreen`、`child-recovered` 三个阶段写 `.ready`，
  等待外部 `screencapture` 完成后写对应 `.continue`；普通自测不启用该人工 checkpoint。

当前未宣称通过：主入口 `--selftest-level` 已接线；`scripts/shell/verify_levels.py` 准备两项 GUI 变异（强制 collection=0、呼出 level=3），尚未运行。必须看到 T50 自身失败才算有效拦截，前置热键失败不能代替。Windows 行为留 Phase 09。

2026-10-03：`--selftest-positioning` 的 T47 前置返回锁屏预检失败（rc=1、未投递事件）；只读复核 CGSSessionScreenIsLocked=1、CGPreflightPostEventAccess=true。T50 未运行，三阶段截图未读取，无独立全屏 / 层级 GUI 拦截证据。

主窗口激活后通过 query 标记要求 child 重新读取原生状态，避免旧 READY 冒充当前 Space；窗口栈必须证明 Buddy 在 child 前，child 重激活后则在 Buddy 前并相交；退出全屏还检查同一主窗口在普通 Space 可用。这些断言已经实现，不能视作已通过。

2026-10-03 静态回归：chat / pages / app / markdown / streaming / settings preview 的 `--selftest`、settings 的 `--selftest-preferences`、shell 的 `--selftest-window` 共 8 组 rc=0 / PASS（`/tmp/preview-s0706-summary.log`）。T45 已在当前定位接线上验证尺寸策略，T46 读回 level=0 / collection=257 等原生属性；锁屏下这些值不能证明真实工作区 / 聚焦 / 系统定位行为。完整 `--selftest` 待解锁后执行。

2026-10-03：暂存后运行 `scripts/gate.sh`，rc=0，输出「gate: 全部通过」（`/tmp/gate-s0706.log`）；门禁不代替上述待验 GUI 行为。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：

## 备注

后续 spec 的能力不计入本项完成。
