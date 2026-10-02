# S07-03 全局热键唤起与切换

> 状态: `done`
> Phase: 07
> 依赖: S00-03
> 阻塞: —
> 退役设计文档: docs/design/rust-architecture.md 热键职责

## 目标

注册配置热键，Pressed / Released 去重，设置保存后更新注册。

## 输入

- `docs/tasks/v2.0.0-gpui/07-shell.md`
- `src-tauri/src/hotkey.rs`
- `src-tauri/src/window/mod.rs`

## 实现要点

- 注册配置热键，Pressed / Released 去重，设置保存后更新注册。
- 冲突 / 失败中文反馈，失败更新保留旧注册；按 v1 核查普通 / 选中文本唤起。
- 依赖计划（先登记后改依赖）：UI 新增 global-hotkey 0.8，沿用 tokio mpsc 无轮询桥接；版本来自 S00-03 的实测组合，不引入 tao / winit 事件循环。实际闭包增量实施后回填。
- 2026-10-03 依赖修订已实施：本机 macOS 26.4 实测与 Apple SDK `CarbonEvents.h` 确认，global-hotkey 0.8.0 的 Carbon options=0 会在外部独占占键时仍注册成功却无法收到事件。已将该 MIT/Apache-2.0 发布包置于 `vendor/global-hotkey/`，仅将 macOS 注册改为 `kEventHotKeyExclusive`；通过 workspace patch 使用，不改 GPUI，不影响独立 v1 workspace，不新增依赖包；独立 owner 冲突、旧键保留及 options=0 对照行为均已纳入验收。
- 产品入口安装进程唯一运行时；PageRouter 的热键保存队列注入系统 updater，注册失败不写盘，写盘失败恢复旧注册。
- 维持 v1 的三态切换与十分钟闲置呼出回紧凑页；隐藏时先发 Cmd+C，50ms 后读选区并恢复剪贴板，只在 empty / noapikey / conversation 写入草稿，不自动发送。

## 验收标准

- [x] 真实系统热键二次切换，更新后旧键失效、新键生效。
- [x] 流式隐藏重开不取消，失败恢复有效拦截。
- [x] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

| 项目 | 可复核材料 / 结果 |
|------|-------------------|
| 2026-10-03 本地实际渲染 | 主 agent 运行手动 `shell_preview` / `--dark`，按本进程实际 CGWindowID 捕获并读取浅 / 深色 560×60 窗口：实色内容、16px 圆角、无标题栏 / 交通灯、中文输入提示与 SVG 操作图标正常。图片仅在 `/tmp/buddy-shell-current-light.png` / `/tmp/buddy-shell-current-dark.png`，未入库；未启用 test-support / render_to_image，预览进程已清理。 |
| 2026-10-03 桌面与完整行为基线 | CGSSessionScreenIsLocked=false、CGPreflightPostEventAccess=true；`NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example shell_preview -- --selftest-behavior` rc=0，`PASS S07-03/S07-04 behavior 自测`。T47 真实 B/N 三态、旧键失效、新键生效、同一 Router 和隐藏后的完整 `0,1,…,79,` 全部为 true；初始激活为异步，等待真实 key 状态而非仅检查 show 返回。 |
| 2026-10-03 真实 OS 冲突 / 回滚 | 独立 Carbon owner 独占旧候选 B 并写 READY；设置录制 B 返回中文注册失败，内存 / 注册 / 磁盘保持 N；owner 收到 B 的 OS ACK，N 仍能隐藏 / 重显。上游 options=0 时该用例复现保存成功却收不到键，exclusive 补丁后八项均 true。随后真实目录故障验证保存失败恢复旧注册与磁盘；沙盒目录已还原，child 和标记文件由 Drop 清理。 |
| 2026-10-03 外部应用选区链路 | T49 专用独立 child TextArea 原生 key READY → 实际 bounds 点击 ACK → OS Cmd+A → child 读取完整 selected_range 后写 SELECTED → B/N 生产热键 → 产品 Cmd+C → 激活主窗口 → trim 后草稿；复制、剪贴板恢复、消息数未变、同一 Router 十项均 true。同进程 Composer 的 OS Cmd+C 与 Settings gate 断言仍保留。之前立即激活时草稿为空，取词完成再显示后通过。 |
| 实现边界 | `crates/ui/src/shell/hotkey.rs` / `hotkey/core.rs` 保持唯一 manager 与事件 channel；`runtime.rs` 安装产品主窗口及三态切换；`router_preferences.rs` 在写盘前更新注册，写盘失败恢复旧键。`selection.rs` 按 v1 同步投递 Cmd+C，再异步等待 50ms 读回并恢复完整剪贴板项目。 |
| 单测 | `cargo test -q -p buddy-ui --lib`：182 passed，新增 15 项（热键事务 / 事件过滤 11、选区规则 3、闲置阈值 1）。FakeBackend 覆盖注册冲突保留旧键、旧键注销失败、新键回滚失败与遗留注册清理；此证据不代替 OS 注册冲突实测。 |
| 有效拦截 | `python3 scripts/shell/verify_behavior.py --case press-dedup --case release-no-toggle --case current-id --case unchanged-key --case cleanup-before-update --case rollback-new-registration --case unregister-old --case selection-trim --case selection-unchanged --case idle-compact`：10/10，故意改坏后均 rc=101 且测试 FAILED；每项 finally 恢复原始字节，未以编译失败计作拦截。 |
| 2026-10-03 最终回归 | UI 单测 182 passed，rc=0。chat / pages / app / markdown / streaming / settings preview 的 `--selftest`、settings 的 `--selftest-preferences`、shell 的 `--selftest-window` 及完整 `--selftest` 均 rc=0 且有 PASS（9 组）。T47 拆分辅助模块后完整 T45～T49 再次通过；T47 唯一 install，T48/T49 复用同一窗口与 Router。日志 `/tmp/ui-final.log`、`/tmp/preview-final-summary.log`、`/tmp/shell-full-final.log`、`/tmp/shell-window-final.log`；settings 修复后的基线在下行变异目录，不能把初轮失败汇总改成成功。 |
| T35 回归时序修正 | 固定 80ms 采样连续失败，诊断为两次样本空串 / “第一”，渐显尚未完成。改为最多 100 轮 `20ms timer + draw` 等待真实 pump，完整第一段到达后才注入第二段，并加强完整“第一段第二段”断言；不手动 tick / flush。`python3 scripts/settings/verify_phase06.py --case streaming-pump`：基线 rc=0 / PASS，禁用 `c.state.tick(now)` 后 rc=1 / FAIL，有效拦截 1/1，脚本 rc=0；源码还原。日志 `/tmp/settings-pump-interception.log`，目录 `buddy-phase06-interception-0x1iaizn`。 |
| 2026-10-03 窗口拦截 | `python3 scripts/shell/verify_window.py > /tmp/window-interception-current.log 2>&1`：23/23，脚本 rc=0；native-focus 在解锁桌面实际触发 FAIL，补齐上轮锁屏时 22/23 的漏检。该脚本验证 S07-01/02，不能替代本项行为链路。 |
| 2026-10-02 锁屏记录（历史） | `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example shell_preview -- --selftest-behavior`：rc=1，输出 `FAIL T47：当前 macOS 会话已锁定，请解锁后重试；未发送系统输入`。只读探测为 CGSSessionScreenIsLocked=1、前台 loginwindow、原生激活返回 false；临时诊断已还原。T47/T48/T49 当时未计为通过；该记录不代表 2026-10-03 已解锁桌面结果。 |
| 2026-10-02 测试准备（历史） | 最初用专用 F18/F19 规划真实 CGEvent 按下 / 释放；实测确认该键型不触发 Carbon 热键后改用 B/N，三态、旧键失效 / 新键生效、实际注册值在写盘失败后回滚、隐藏期间慢 SSE 的终态 79 等断言均保留。T49 通过实际 Composer 选区和 OS Cmd+C 读回文本，检查旧剪贴板恢复和草稿不发送。 |
| 有效拦截边界 | 原整轮首轮结果为 24/25、rc=1，唯一 `hotkey-save-rollback` 因变异编译失败不计有效；修正为 `Ok::<(), String>(())` 后单项基线 PASS 且真实行为 FAIL、rc=0，补足独立有效证据，最终 25/25。两轮日志分别为 `/tmp/behavior-interception.log`、`/tmp/behavior-rollback-interception.log`；不将首轮整轮 rc=1 改写为整轮 rc=0。每项源码均在 finally 后还原。 |
| 证据缺口 | 真实十分钟闲置切页、进程在 50ms 内退出时的剪贴板恢复仍无独立自动化证据；非 macOS 原生显隐尚未实现，Windows 实测归 Phase 09。 |
| 2026-10-03 提交门禁 | 暂存后按代理环境运行 `scripts/gate.sh > /tmp/gate.log 2>&1`：rc=0，输出 `gate: 全部通过`（含 workspace / v1 编译）。实现提交 `2ddf6d6`，仅本地提交，未 push。 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |
| 事件桥 | 主线程持久 manager + 唯一 handler + tokio mpsc | 沿用 S00-03 已实测组合，去掉轮询延迟；系统回调不借用 GPUI App，主线程消费时匹配当前 ID 并去重。 |
| 注册事务 | 新键注册成功后才注销旧键；清理遗留注册先于转换 | 若任何步骤失败，当前有效配置不能已被换成未发布的新键。旧键注销失败回滚新键，回滚失败明确记录并可重试清理。 |
| 写盘失败的偏离 v1 | 系统热键恢复到保存前值 | 当前 v1 的 commands::save_config 在注册成功后写盘，磁盘失败未回滚；新实现补齐回滚，避免界面 / 磁盘旧键与实际新键不一致。 |
| 剪贴板恢复的偏离 v1 | 保存 / 恢复完整 GPUI ClipboardItem | 当前 v1 只恢复旧文本，可能覆盖原有图片；使用既有 GPUI API，不引入第二套剪贴板依赖。OS 取词与时序仍需真实事件证据。 |
| macOS 冲突修复（偏离 v1） | vendored global-hotkey 0.8.0 仅 Carbon options 改为独占 | 本轮独立 owner 实测：上游非独占注册在其他进程独占占键时仍返回成功且不收事件；Apple SDK CarbonEvents.h HotKeyOptions 明确说明该行为。使用独占注册才能在保存前返回中文错误并保留旧键。来源与复核见 `vendor/global-hotkey/VENDOR.md`，v1 独立 workspace 不改。 |
| 外部取词时序（偏离 v1） | 完成 50ms 取词与剪贴板恢复后才显示 / 激活 | CGEventPost 只是排队，立即激活会抢走外部应用的 Cmd+C；本轮 T49 同进程复制通过而外部唤起草稿为空，不能用前者代替完整链路。取词结束再激活也避免同一热键桥的多次取词重叠。 |
| 系统自测键 | 专用 CmdOrCtrl+Alt+Shift+B/N | macOS 26.4 独立 AppKit/Carbon 控制程序中，合成 F18 只到 NSEvent、不触发 Carbon 热键，显式 HID 源亦相同；同程序 B 可触发。仅替换测试键型，旧键失效 / 新键生效 / 释放不重复切换等断言全部保留；实体 F18/F19 无独立证据。 |
| 依赖增量 | Cargo.lock 增加 75 行 / 7 个包；block 已在闭包中 | `cargo tree -p buddy-ui --target aarch64-apple-darwin --format '{p}' --prefix none` 的新增可达包仅 global-hotkey / crossbeam-channel / keyboard-types；gethostname / x11rb / x11rb-protocol / xkeysym 仅进入 lock。未引入 tao / winit / gtk，objc2 与 app-kit 保持 GPUI 既有版本。 |

## 完成记录

- 日期：2026-10-03
- commit：`2ddf6d6`（本轮修复与系统验收；基础实现 `80f6660`）
- 设计文档处置：rust-architecture.md 热键职责 / 状态及 pages-and-states.md 热键 / 选区段落已部分退役；已登记 design-deletions.md 的“已部分删减的文档”。

## 备注

后续 spec 的能力不计入本项完成。
