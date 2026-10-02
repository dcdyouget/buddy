# S07-04 Esc / 点击外部关闭（不断流）

> 状态: `blocked`
> Phase: 07
> 依赖: S07-02
> 阻塞: 等待解锁 macOS 会话，运行真实外部点击 / 聚焦切换自测；不是等待用户目检。
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
| 实现边界 | `visibility.rs` 在主线程调用原生 orderOut / show，NSEvent global monitor 仅将其他应用的左 / 右 / 中键按下投递 channel。`shell/mod.rs` 根级 Esc 在子层未消费时异步请求隐藏；`PageRouter::prepare_window_hide` 关闭独立模型菜单并通知 Conversation 隐藏，不销毁 Router 或停止 engine。 |
| 回归 | `cargo test -q -p buddy-ui --lib`：182 passed。chat_preview / pages_preview / app_preview / markdown_preview / streaming_preview / settings_preview 的 `--selftest`、settings_preview 的 `--selftest-preferences`、shell_preview 的 `--selftest-window` 共 8 组 rc=0 且有 PASS；后者按真实窗口读回尺寸、标题栏 / 阴影 / layer 等原生属性。命令均设置 `NO_PROXY=127.0.0.1,localhost`。 |
| 已备 T48 | 同一生产运行时上，用真实 `window.dispatch_event` 验证普通 Esc、普通设置页 Esc 隐藏，Provider 返回、热键录制取消、模型菜单关闭、审批拒绝的优先级；审批 mock 的绝对路径在沙盒内并检查没有写出文件。专用 child 写 ready 和实际 bounds，OS 鼠标点到该 child 后写 ACK；主窗口外点隐藏期间不 draw，等待真实慢 SSE 到终态 79，再显示并检查同一 Router 和完整内容。 |
| 当前阻塞 | `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example shell_preview -- --selftest-behavior`：rc=1；T47 的锁屏预检在任何系统键鼠投递前停止，因此 T48/T49 未执行。CGSSessionScreenIsLocked=1、前台 loginwindow，不能把聚焦失败当作本项有效拦截。临时原生诊断已还原。 |
| 拦截边界 | `scripts/shell/verify_behavior.py` 已备原生隐藏、外点 mask / delivery、根 Esc、审批 Esc 优先级与误取消生成变异；依赖系统行为的 11 项尚未运行，本项无独立自动化证据。S07-03 的纯逻辑 10/10 拦截不冒充本项物理输入证据。 |
| 既有窗口拦截复跑 | `python3 scripts/shell/verify_window.py` 改为定向 `--selftest-window` 后仍为 22/23，native-focus 漏检如实保留，脚本 rc=1；此结果只验证 S07-01/02 的原生属性 / 尺寸，不证明本项显隐和外点行为。 |
| 提交门禁 | 暂存后按代理环境运行 `scripts/gate.sh > /tmp/gate.log 2>&1`：rc=0，输出 `gate: 全部通过`（含 workspace / v1 编译）。未 push；本次提交保存待验实现，不填完成记录。 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |
| 外部点击的偏离 v1 | 只监听真实外部鼠标按下并隐藏 | 当前 v1 已移除原生失焦自动隐藏；硬约束 7 要求点击外部关闭，因此补齐外点路径。不使用 resignKey，避免唤起竞态和模型菜单焦点变化误隐藏。 |
| Esc 优先级 | 子层先消费，剩余冒泡到主窗口根 | 模型菜单关闭、审批拒绝、Provider 返回、热键录制取消后都保持主窗口。普通设置页无独立 Esc 返回行为，沿当前 v1 根级 Esc 隐藏并保留当前设置页。 |
| 隐藏与生成隔离 | 原生 orderOut，显隐直接通知 Conversation 缓冲 | 不销毁 Router，不调用 stop_generation；隐藏窗口是否出帧不影响真实 engine 任务及终态存储。独立模型菜单在主窗口隐藏前关闭。 |
| 原生时机 | prepare 只取强引用，退出 App 借用后同步显隐 | AppKit 显隐 / 激活同步回调可重入 GPUI；短暂引用不跨 await，原生事件回调只投递 channel。 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：保留 Global Interactions；真实行为验收通过后再按 RULES §7 部分退役并登记。

## 备注

后续 spec 的能力不计入本项完成。
