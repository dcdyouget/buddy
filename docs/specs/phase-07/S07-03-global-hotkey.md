# S07-03 全局热键唤起与切换

> 状态: `blocked`
> Phase: 07
> 依赖: S00-03
> 阻塞: 等待解锁 macOS 会话，运行真实系统热键 / 聚焦 / Cmd+C 自测；不是等待用户目检。
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
- 产品入口安装进程唯一运行时；PageRouter 的热键保存队列注入系统 updater，注册失败不写盘，写盘失败恢复旧注册。
- 维持 v1 的三态切换与十分钟闲置呼出回紧凑页；隐藏时先发 Cmd+C，50ms 后读选区并恢复剪贴板，只在 empty / noapikey / conversation 写入草稿，不自动发送。

## 验收标准

- [ ] 真实系统热键二次切换，更新后旧键失效、新键生效。
- [ ] 流式隐藏重开不取消，失败恢复有效拦截。
- [ ] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

| 项目 | 可复核材料 / 结果 |
|------|-------------------|
| 实现边界 | `crates/ui/src/shell/hotkey.rs` / `hotkey/core.rs` 保持唯一 manager 与事件 channel；`runtime.rs` 安装产品主窗口及三态切换；`router_preferences.rs` 在写盘前更新注册，写盘失败恢复旧键。`selection.rs` 按 v1 同步投递 Cmd+C，再异步等待 50ms 读回并恢复完整剪贴板项目。 |
| 单测 | `cargo test -q -p buddy-ui --lib`：182 passed，新增 15 项（热键事务 / 事件过滤 11、选区规则 3、闲置阈值 1）。FakeBackend 覆盖注册冲突保留旧键、旧键注销失败、新键回滚失败与遗留注册清理；此证据不代替 OS 注册冲突实测。 |
| 有效拦截 | `python3 scripts/shell/verify_behavior.py --case press-dedup --case release-no-toggle --case current-id --case unchanged-key --case cleanup-before-update --case rollback-new-registration --case unregister-old --case selection-trim --case selection-unchanged --case idle-compact`：10/10，故意改坏后均 rc=101 且测试 FAILED；每项 finally 恢复原始字节，未以编译失败计作拦截。 |
| 回归 | 设置 `NO_PROXY=127.0.0.1,localhost` 后，chat_preview / pages_preview / app_preview / markdown_preview / streaming_preview / settings_preview 的 `--selftest`，settings_preview 的 `--selftest-preferences`、shell_preview 的 `--selftest-window` 共 8 组均 rc=0 且有 PASS；UI 182 单测通过。 |
| 既有窗口拦截复跑 | `python3 scripts/shell/verify_window.py` 已定向使用 `--selftest-window`，避免窗口属性证据依赖后续物理输入；22/23，有效项均触发测试 FAIL，native-focus 仍漏检，脚本 rc=1；不把该漏检作为通过。 |
| 系统行为待验 | `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example shell_preview -- --selftest-behavior`：rc=1，输出 `FAIL T47：当前 macOS 会话已锁定，请解锁后重试；未发送系统输入`。只读探测为 CGSSessionScreenIsLocked=1、前台 loginwindow、原生激活返回 false；临时诊断已还原。T47/T48/T49 未计为通过。 |
| 已备测试 | T47 用专用 F18/F19 真实 CGEvent 按下 / 释放，检查三态切换、真实录制后的旧键失效 / 新键生效、实际注册值在写盘失败后回滚、隐藏期间慢 SSE 的终态 79；T49 通过实际 Composer 选区和 OS Cmd+C 读回文本，检查旧剪贴板恢复和草稿不发送。 |
| 证据缺口 | 11 项依赖 GUI 行为的拦截尚未运行；外部应用选区在唤起 / 激活交错下的复制成功率、OS 热键冲突、真实十分钟闲置切页、进程在 50ms 内退出时的剪贴板恢复无独立自动化证据。非 macOS 原生显隐尚未实现，Windows 实测归 Phase 09。 |
| 提交门禁 | 暂存后按代理环境运行 `scripts/gate.sh > /tmp/gate.log 2>&1`：rc=0，输出 `gate: 全部通过`（含 workspace / v1 编译）。未 push；本次提交保存待验实现，不填完成记录。 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |
| 事件桥 | 主线程持久 manager + 唯一 handler + tokio mpsc | 沿用 S00-03 已实测组合，去掉轮询延迟；系统回调不借用 GPUI App，主线程消费时匹配当前 ID 并去重。 |
| 注册事务 | 新键注册成功后才注销旧键；清理遗留注册先于转换 | 若任何步骤失败，当前有效配置不能已被换成未发布的新键。旧键注销失败回滚新键，回滚失败明确记录并可重试清理。 |
| 写盘失败的偏离 v1 | 系统热键恢复到保存前值 | 当前 v1 的 commands::save_config 在注册成功后写盘，磁盘失败未回滚；新实现补齐回滚，避免界面 / 磁盘旧键与实际新键不一致。 |
| 剪贴板恢复的偏离 v1 | 保存 / 恢复完整 GPUI ClipboardItem | 当前 v1 只恢复旧文本，可能覆盖原有图片；使用既有 GPUI API，不引入第二套剪贴板依赖。OS 取词与时序仍需真实事件证据。 |
| 依赖增量 | Cargo.lock 增加 75 行 / 7 个包；block 已在闭包中 | `cargo tree -p buddy-ui --target aarch64-apple-darwin --format '{p}' --prefix none` 的新增可达包仅 global-hotkey / crossbeam-channel / keyboard-types；gethostname / x11rb / x11rb-protocol / xkeysym 仅进入 lock。未引入 tao / winit / gtk，objc2 与 app-kit 保持 GPUI 既有版本。 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：保留热键职责段；真实系统行为验收通过后再按 RULES §7 退役并登记。

## 备注

后续 spec 的能力不计入本项完成。
