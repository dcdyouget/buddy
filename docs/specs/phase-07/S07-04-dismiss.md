# S07-04 Esc / 点击外部关闭（不断流）

> 状态: `doing`
> Phase: 07
> 依赖: S07-02
> 阻塞: —
> 退役设计文档: docs/design/pages-and-states.md Global Interactions

## 目标

当前 v1 已因唤起竞态移除原生失焦自动隐藏，先核查实际交互，不照搬旧素材。

## 输入

- `docs/tasks/v2.0.0-gpui/07-shell.md`
- `src-tauri/src/window/events.rs`
- `src/App.tsx`
- `docs/design/pages-and-states.md`

## 实现要点

- 当前 v1 已因唤起竞态移除原生失焦自动隐藏，先核查实际交互，不照搬旧素材。
- 落实硬约束 Esc / 点击外部；菜单 / 审批 / 热键录制优先消费 Esc，工具弹窗焦点变化不误判。
- 原生隐藏，不销毁 Router，不 stop_generation。
- 依赖预案（先登记后改依赖）：macOS NSEvent 鼠标全局监听如需 Objective-C block，使用 GPUI 闭包已有 block 0.1；不使用原生失焦回调作为外部点击替身。

## 验收标准

- [ ] 真实外部点击 / Esc / 弹窗焦点变化，慢流式隐藏重显内容和终态完整。
- [ ] 误取消生成和错误 Esc 优先级有独立拦截。
- [ ] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

| 项目 | 可复核材料 / 结果 |
|------|-------------------|
| 2026-10-03 本地实际渲染 | 主 agent 运行手动 `shell_preview` / `--dark`，按本进程实际 CGWindowID 捕获并读取浅 / 深色 560×60 窗口：实色内容、16px 圆角、无标题栏 / 交通灯、中文输入提示与 SVG 操作图标正常。图片仅在 `/tmp/buddy-shell-current-light.png` / `/tmp/buddy-shell-current-dark.png`，未入库；未启用 test-support / render_to_image，预览进程已清理。 |
| 2026-10-03 完整行为基线 | 解锁且 CGEvent 权限通过后，`shell_preview --selftest-behavior` rc=0、`PASS S07-03/S07-04 behavior 自测`。T48 流式普通 Esc、设置页 Esc、Provider 返回、录制取消、模型菜单关闭、审批拒绝优先级及外点链路均 true；审批使用独立绝对路径沙盒，并在流停止后确认文件没有写出。 |
| 2026-10-03 原生外点 / 不断流 | 同一主窗口和 Router；专用 child 实际 key READY → 解析实际 bounds 点击中心 → 鼠标 ACK → 主窗口隐藏；隐藏等待阶段无显式 draw，完整比对 0～79 全部 token 和终态后重显。不是 resignKey 模拟、不是手工 stop/start。 |
| 2026-10-03 缺陷复现与修复 | 流式开始后输入框被“生成中”占位元素替换，原 focus 节点从分发树消失，T48 普通 Esc 为 false。`composer.rs` 在流式占位区域追踪原输入焦点，保留冒泡链且没有启用编辑；修复后该断言与其余优先级同时通过。 |
| 实现边界 | `visibility.rs` 在主线程调用原生 orderOut / show，NSEvent global monitor 仅将其他应用的左 / 右 / 中键按下投递 channel。`shell/mod.rs` 根级 Esc 在子层未消费时异步请求隐藏；`PageRouter::prepare_window_hide` 关闭独立模型菜单并通知 Conversation 隐藏，不销毁 Router 或停止 engine。 |
| 2026-10-03 最终回归 | UI 单测 182 passed，rc=0。chat / pages / app / markdown / streaming / settings preview 的 `--selftest`、settings 的 `--selftest-preferences`、shell 的 `--selftest-window` 及完整 `--selftest` 均 rc=0 且有 PASS（9 组）。T47 拆分辅助模块后完整 T45～T49 再次通过；T47 唯一 install，T48/T49 复用同一窗口与 Router。日志 `/tmp/ui-final.log`、`/tmp/preview-final-summary.log`、`/tmp/shell-full-final.log`、`/tmp/shell-window-final.log`；settings 修复后的基线在下行变异目录，不能把初轮失败汇总改成成功。 |
| T35 回归时序修正 | 固定 80ms 采样连续失败，诊断为两次样本空串 / “第一”，渐显尚未完成。改为最多 100 轮 `20ms timer + draw` 等待真实 pump，完整第一段到达后才注入第二段，并加强完整“第一段第二段”断言；不手动 tick / flush。`python3 scripts/settings/verify_phase06.py --case streaming-pump`：基线 rc=0 / PASS，禁用 `c.state.tick(now)` 后 rc=1 / FAIL，有效拦截 1/1，脚本 rc=0；源码还原。日志 `/tmp/settings-pump-interception.log`，目录 `buddy-phase06-interception-0x1iaizn`。 |
| 2026-10-02 测试准备 | 同一生产运行时上，用真实 `window.dispatch_event` 验证普通 Esc、普通设置页 Esc 隐藏，Provider 返回、热键录制取消、模型菜单关闭、审批拒绝的优先级；审批 mock 的绝对路径在沙盒内并检查没有写出文件。专用 child 写 ready 和实际 bounds，OS 鼠标点到该 child 后写 ACK；主窗口外点隐藏期间不 draw，等待真实慢 SSE 到终态 79，再显示并检查同一 Router 和完整内容。 |
| 2026-10-02 锁屏记录（历史） | `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example shell_preview -- --selftest-behavior`：rc=1；T47 的锁屏预检在任何系统键鼠投递前停止，因此 T48/T49 未执行。CGSSessionScreenIsLocked=1、前台 loginwindow，不能把聚焦失败当作本项有效拦截。临时原生诊断已还原；该记录不代表 2026-10-03 已解锁桌面结果。 |
| 有效拦截边界 | 原整轮首轮结果为 24/25、rc=1，唯一 `hotkey-save-rollback` 因变异编译失败不计有效；修正为 `Ok::<(), String>(())` 后单项基线 PASS 且真实行为 FAIL、rc=0，补足独立有效证据，最终 25/25。两轮日志分别为 `/tmp/behavior-interception.log`、`/tmp/behavior-rollback-interception.log`；不将首轮整轮 rc=1 改写为整轮 rc=0。每项源码均在 finally 后还原。2026-10-03 解锁后窗口整轮也完成 23/23 有效拦截，见下行。 |
| 2026-10-03 窗口拦截 | `python3 scripts/shell/verify_window.py > /tmp/window-interception-current.log 2>&1`：23/23，脚本 rc=0；native-focus 在解锁桌面实际触发 FAIL，补齐上轮锁屏时 22/23 的漏检。该脚本验证 S07-01/02，不能替代本项行为链路。 |
| 上轮提交门禁 | 暂存后按代理环境运行 `scripts/gate.sh > /tmp/gate.log 2>&1`：rc=0，输出 `gate: 全部通过`（含 workspace / v1 编译）。未 push；本次提交保存待验实现，不填完成记录。 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |
| 外部点击的偏离 v1 | 只监听真实外部鼠标按下并隐藏 | 当前 v1 已移除原生失焦自动隐藏；硬约束 7 要求点击外部关闭，因此补齐外点路径。不使用 resignKey，避免唤起竞态和模型菜单焦点变化误隐藏。 |
| Esc 优先级 | 子层先消费，剩余冒泡到主窗口根 | 模型菜单关闭、审批拒绝、Provider 返回、热键录制取消后都保持主窗口。普通设置页无独立 Esc 返回行为，沿当前 v1 根级 Esc 隐藏并保留当前设置页。 |
| 隐藏与生成隔离 | 原生 orderOut，显隐直接通知 Conversation 缓冲 | 不销毁 Router，不调用 stop_generation；隐藏窗口是否出帧不影响真实 engine 任务及终态存储。独立模型菜单在主窗口隐藏前关闭。 |
| 流式焦点树 | 生成中占位区域追踪原 Composer 输入框焦点 | 生成时 TextArea 从元素树中被替换，原焦点失去祖先链，导致根 Esc 收不到事件。保持同一焦点 ancestry，不启用输入、不取消生成；T48 的普通流式 Esc 是复现入口。 |
| 原生时机 | prepare 只取强引用，退出 App 借用后同步显隐 | AppKit 显隐 / 激活同步回调可重入 GPUI；短暂引用不跨 await，原生事件回调只投递 channel。 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：Global Interactions 中 Esc / 外点 / 热键与选区段落已部分退役，保留 S07-06 定位；已登记 design-deletions.md 的“已部分删减的文档”。

## 备注

后续 spec 的能力不计入本项完成。
