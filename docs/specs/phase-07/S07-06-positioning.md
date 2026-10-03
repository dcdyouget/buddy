# S07-06 多显示器与窗口定位

> 状态: `done`
> Phase: 07
> 依赖: S07-01
> 阻塞: —
> 退役设计文档: docs/design/rust-architecture.md SavedWindowPositions / 定位职责

## 目标

移植鼠标所在屏幕、每屏运行期位置记忆、工作区裁剪和保存防抖。

## 输入

- `docs/tasks/v2.0.0-gpui/07-shell.md`
- `src-tauri/src/window/geometry.rs`
- `src-tauri/src/window/positioning.rs`
- `src-tauri/src/window/events.rs`

## 实现要点

- 移植鼠标所在屏幕、每屏运行期位置记忆、工作区裁剪和保存防抖。
- 全局热键沿用 v1 的焦点屏优先；托盘 / 鼠标呼出沿用 v1 的鼠标屏优先。
- 区分 GPUI / AppKit 原点，内部统一全局左上逻辑点，处理负坐标 / 屏幕拔插；窗口尺寸全程使用 AppKit 逻辑点，不手动缩放。
- 内容增高固定底边，尺寸调用 S07-01 统一策略。
- 呼出定位沿用 v1：saved 位置只有在完整显示器 frame 内才复用，否则在完整 frame 居中；visible work area 与边距只用于页面 resize 的裁剪。

## 验收标准

- [x] 几何单测覆盖 DPI、负坐标、边界和底边锚定，各分支有效拦截。
- [x] 真实单屏定位；无法接入多屏硬件明确证据缺口。
- [x] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

- `crates/ui/src/shell/positioning.rs` / `positioning/tests.rs`：DPI 逻辑点、负坐标、左下 / 左上往返、屏幕来源回退、全屏 frame 居中与无效 saved 回退、resize 工作区裁剪和底边锚定。
- `positioning_native.rs` / `positioning_native/ffi.rs`：公开 AppKit NSWindow / NSScreen、CGDisplay UUID；不新增依赖、不改 GPUI。`positioning_controller.rs` 接线移动 160ms 防抖、失活 / 隐藏立即保存、待页面尺寸提交后再按新尺寸呼出；原生 setFrame 在退出 GPUI 借用后执行。
- `cargo test -q -p buddy-ui --lib`：191 passed，rc=0，无编译 warning（`/tmp/positioning-unit-final.log`）。
- `scripts/shell/verify_positioning.py` 的 9 个纯逻辑 case：修正 v1 唤起语义后重新跑基线，9/9 均通过故意改坏 → 测试 FAILED（rc=101）→ 原始字节还原；脚本 rc=0，日志 `/tmp/positioning-interception-final.log`。覆盖原点、裁剪最小 / 最大 / 超大目标、中心 / 底边、saved、屏幕 fallback、位置保存；之前版本首轮 9/9 不替代本轮证据。
- 2026-10-03 解锁后 `shell_preview --selftest-positioning` rc=0 / PASS（`/tmp/positioning-current.log`）：T47–T49 与 T51 全部通过，真实移动 / 160ms 防抖、隐藏移走后恢复、底边 / 中心 / 工作区、用户尺寸及同 Router 均为 true。实机 scale=2、工作区 (0,30,1920,1013)，紧凑 (270,843,560,60) → 设置 (170,263,760,640)，底边均为 903、中心 x 均为 550。
- GUI 有效拦截 4/4，分两轮：`/tmp/positioning-gui-interception-current.log` 的 save-debounce / save-observer / show-restore 三项确实进入 T51 并失败；原 page-geometry 只触发 T47 前置失败，虽然旧脚本报告 4/4，**该项不计有效**。脚本现强制包含 T51 输出，page-geometry 改为保持尺寸但破坏底边锚定；`/tmp/positioning-page-interception-current.log` 单独重跑 1/1，T47–T49 PASS、T51 锚定 / 工作区 false，脚本 rc=0。所有源码已按原始字节还原。
- `python3 scripts/shell/verify_window.py --case resize-subscription`：更新尺寸接线锚点后基线 PASS，故意移除 controller 调用触发 T45 真实尺寸 FAIL，1/1 有效、脚本 rc=0（`/tmp/positioning-resize-interception.log`）。该项不投递 OS 输入，不能代替 T51。
- 多屏 / 混合 DPI / 屏幕拔插目前只有纯逻辑覆盖，无真实硬件证据；单屏原生定位与实际渲染已完成。

2026-10-03 静态回归：chat / pages / app / markdown / streaming / settings preview 的 `--selftest`、settings 的 `--selftest-preferences`、shell 的 `--selftest-window` 共 8 组 rc=0 / PASS（`/tmp/preview-s0706-summary.log`）。T45 已在当前定位接线上验证尺寸策略，T46 读回 level=0 / collection=257 等原生属性；锁屏下这些值不能证明真实工作区 / 聚焦 / 系统定位行为。完整 `--selftest` 待解锁后执行。

2026-10-03：暂存后运行 `scripts/gate.sh`，rc=0，输出「gate: 全部通过」（`/tmp/gate-s0706.log`）；门禁不代替上述待验 GUI 行为。

2026-10-03 实际渲染：设置 `BUDDY_SHELL_T51_CAPTURE_DIR=/tmp/buddy-t51-current` 运行同一 T51，在 compact-restored / expanded 的 ready 检查点用 `screencapture -x` 采样，读取 `/tmp/buddy-t51-compact-restored.png` 与 `/tmp/buddy-t51-expanded.png`。紧凑气泡与设置实色圆角面板的底边保持一致、左右对称展开，文字与控件完整，未越过工作区；截图不入库。带检查点完整自测 rc=0 / PASS（`/tmp/positioning-capture-current.log`）。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |
| 屏幕来源 | 焦点屏 / 鼠标屏分别沿用 v1 | 全局热键使用 `NSScreen.mainScreen`；鼠标呼出使用 `NSEvent.mouseLocation`；两者均按当前窗口屏幕再回退 primary。 |
| 呼出 frame | 完整 frame 居中与 saved 合法性检查 | v1 的初始 / saved 定位不按 Dock / 菜单栏 visible frame 裁剪；工作区裁剪仅用于页面尺寸变化。 |
| 运行期 key | CGDisplay UUID | v1 使用位置坐标作 key；v2 仅运行期记忆，不需兼容历史磁盘数据，UUID 可稳定应对显示器重排。 |
| 坐标与 DPI | AppKit 逻辑点，统一左上内部坐标 | 原生桥负责 AppKit 左下坐标转换；不把 backing scale 重复乘入窗口尺寸。 |

本轮暂存后 `scripts/gate.sh` rc=0（`/tmp/gate-s0706-complete.log`）。S07-05 全屏覆盖失败独立记录，不阻断本项单屏定位验收，也不计为完整 shell 自测通过。

## 完成记录

- 日期：2026-10-03
- commit：`d9108e5`（定位实现）；本轮补齐解锁后的系统验收、GUI 拦截及渲染证据。
- 设计文档处置：删除 rust-architecture.md 的 SavedWindowPositions / 定位职责与 pages-and-states.md 定位段，已在 design-deletions.md 的「已部分删减的文档」登记，其余生命周期 / 托盘保留。

## 备注

后续 spec 的能力不计入本项完成。
