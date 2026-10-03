# S07-05 置顶与全工作区可见

> 状态: `done`
> Phase: 07
> 依赖: S07-02
> 阻塞: —
> 退役设计文档: —

## 目标

唤起时沿用 v1 普通 level 0 + 前置 / 激活；外部应用重新激活后不持续覆盖。

## 输入

- `docs/tasks/v2.0.0-gpui/07-shell.md`
- `src-tauri/src/platform/macos.rs`
- `src-tauri/src/platform/windows.rs`
- `src-tauri/tauri.conf.json`

## 实现要点

- 唤起时沿用 v1 普通 level 0 + 前置 / 激活；外部应用重新激活后不持续覆盖。
- 探测 AllSpaces / FullScreenAuxiliary，协调独立模型菜单；Windows 实测留 Phase 09。

## 验收标准

- [x] 唤起前置但不长期压住其他应用；跨桌面 / 全屏空间有实际证据。
- [x] 实际原生层级和工作区读回，关键分支有效拦截。
- [x] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

- `fullscreen_target.rs` 是专用独立普通窗口进程，通过真实全屏 Space，只有 native fullscreen、key window、NSApplication.isActive、isOnActiveSpace 和整屏 bounds 都成立才写 READY。初版 `titlebar: None` 导致 GPUI 不设置 Resizable、全屏静默失败；改为 Some(Default) 后真实全屏成功。未更改产品无装饰配置。
- `workspaces.rs` 从真实 raw window handle 读窗口号、AppKit Space / fullscreen / app-active；`level_native.rs` 以该窗口号查询 CGWindowList 的真实栈与相交 bounds，不用位置猜身份。T50 复用 T47 安装的唯一主窗口 / Router，并要求 child 在 Buddy 激活后重新响应 query，避免旧 READY 冒充当前状态。
- 实测发现：PopUp NSPanel 即使 level=0，仍会在全屏 child 重新激活后覆盖它。单独 orderBack、去掉 FullScreenAuxiliary 后 orderBack 均未修复；改普通 NSWindow 的隔离实验使唤起时 active_space=false。所有无效实验及临时日志已还原，没有修改断言制造通过。
- `focus_order.rs` 订阅真实窗口激活变化；合并旧任务，在 1 秒内分四次重新读取当前 key / app-active / visible，避免 resignKey 早于应用失活的交错。外部应用激活时仅降到 level=-1，仍保留 visible、0x101、同一 Router 与流式；同应用模型菜单转焦不降层。外部 app-inactive 优先于暂存的 key 状态，避免激活交错时错误恢复普通层级；该短暂原生状态没有独立自动化复现证据。原生重新聚焦及 `show_and_focus` 共用恢复 level=0 的方法，避免直接点击重新激活后仍低于普通窗口。
- `/tmp/level-lower.log`：T47–T49 / T50 rc=0 / PASS；Buddy 唤回时 level=0、collection=0x101、active_space=true，位于 child 前；child 再激活时 Buddy level=-1、visible=true、active=false、active_space=true，child window number 位于其前且相交；退出全屏后原主窗口普通 Space 可用。
- 实际渲染：设置 `BUDDY_SHELL_T50_CAPTURE_DIR=/tmp/buddy-t50-final`，各阶段 `.ready` 后 `screencapture -x` 并写 `.continue`；已读取 `/tmp/buddy-t50-final-fullscreen-child.png`、`/tmp/buddy-t50-final-buddy-in-fullscreen.png`、`/tmp/buddy-t50-final-child-recovered.png`，分别为全屏目标、Buddy 前置、目标重新覆盖 Buddy。带截图检查点自测 rc=0（`/tmp/level-final-capture.log`），图片不入库。
- T48 增加同应用模型菜单保持主窗 level=0 的原生读回；已通过。T50 另增加绕过 runtime::show 的原生重新激活恢复 level=0 验证，拦截基线已通过。
- `python3 scripts/shell/verify_levels.py` rc=0，5/5 有效拦截（`/tmp/level-interception-final.log`）：native-focus-restore / external-deactivation-level / all-spaces-collection 均到 T50 后按对应字段 FAIL；same-app-focus-level / normal-show-level 由 T48 新增的原生 level=0 断言明确返回 false，不把任意前置失败算作拦截。基线完整 PASS，逐项故意改坏后均已还原原始字节。
- 最终激活分支顺序修正后，定向重跑 native-focus-restore / same-app-focus-level / external-deactivation-level，3/3 有效拦截，两个 GUI 基线通过（`/tmp/level-focus-order-interception.log` rc=0）。
- 191 UI 单测通过；8 组独立预览回归 rc=0（`/tmp/preview-s0705-final-summary.log`）。首次完整 shell 串联因 T45/T46 遗留窗口及全屏 child 退出后的焦点交错使 T51 失败；现按窗口身份清理本自测创建的窗口、先结束 child 再激活主窗，T51 增加有界聚焦前置，不放宽原定位断言。重跑完整 `shell_preview --selftest` rc=0 / 全部 PASS（`/tmp/shell-full-final.log`），合计 9 组预览有效通过。
- 当前系统有两块 scale=2 的虚拟显示器，完整串联还在负坐标屏完成 T51；这不替代物理多屏 / 混合 DPI / 拔插证据。Windows 留用户后续执行。提交门禁 rc=0（`/tmp/gate-s0705-final.log`）。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 层级偏离 v1 | 唤起 / 聚焦为 0，外部失活为 -1 | 当前 v1 NSWindow 只设普通层级；GPUI NSPanel 在全屏 Space 中普通 level=0 仍覆盖外部窗口。实测需外部失活时降一级才能实现“不永久覆盖”；不隐藏、不取消流式，不使用私有 CGS。 |
| 保留 PopUp | 保留全工作区 0x101 与 NonactivatingPanel | Normal 隔离实验无法在全屏 Space 保持可用，既有原生外观与首次点击语义不变。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |

## 完成记录

- 日期：2026-10-03
- commit：`888a7ae`
- 设计文档处置：本项未声明退役设计段落。

## 备注

后续 spec 的能力不计入本项完成。
