# S07-11 单实例与生命周期（无窗口存活 / 休眠唤醒）

> 状态: `done`
> Phase: 07
> 依赖: S07-01
> 阻塞: —（真实系统睡眠通知未执行，按验收标准单列边界）
> 退役设计文档: 原 rust-architecture.md 生命周期 / 剩余职责与依赖（已退役，见 `docs/specs/design-deletions.md`）

## 目标

第二实例向已有实例发送唤起，不并行写数据；核查 v1 激活策略 / Dock。

## 输入

- `docs/tasks/v2.0.0-gpui/07-shell.md`
- `src-tauri/src/lib.rs`
- `src-tauri/src/window/mod.rs`
- 原 Rust 架构设计（已退役，追溯见 `docs/specs/design-deletions.md`）

## 实现要点

- 第二实例向已有实例发送唤起，不并行写数据；核查 v1 激活策略 / Dock。
- 无可见窗口仍保持 engine / 注册；退出清理资源；唤醒复核热键和定位。

## 验收标准

- [x] 双进程、隐藏继续流式、退出资源释放可复现并有效拦截。
- [x] 真实休眠唤醒缺口如实记录。
- [x] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

- 产品入口先获取用户私有目录中的 flock owner，再创建 engine / GPUI 窗口 / 热键 / 托盘。第二进程通过 Unix socket 发送 Wake，收到 ACK 才退出；仅持有 flock 的 owner 可清理 stale socket，锁文件保留避免 inode 竞态。
- `lifecycle_probe` 首轮 rc=0（`/tmp/lifecycle-probe.log`）：专用 child READY 后启动第二进程，Forwarded + owner Wake ACK；只 kill 专用 owner 模拟崩溃，stale socket 恢复；正常 Drop 清理 socket、保留锁文件并可再获取。探针目录与专用子进程清理。
- `on_system_wake` 重新注册同一热键并清理 Pressed 状态，注册失败不能以旧 current 冒充有效；重新读取屏幕工作区并裁剪原生位置，不调用 show、不改变显隐。托盘退出在调用 quit 前等配置队列；`on_app_quit` 同步注销热键和外点监听 / tray，原生退出使用 Cancel → 排空配置队列 → Now 重试，避免 GPUI 200ms shutdown 上限截断设置；IPC 清理后锁保留到内核终止进程，避免旧进程残余写入与新 owner 重叠。
- `verify_lifecycle.py` 最终 rc=0（`/tmp/lifecycle-interception-final.log`）：本项 owner-lock / stale-socket / socket-cleanup / exit-lock / rearm-unregister / rearm-pressed / wake-registration / quit-hook 共 8/8 有效拦截；另含 S07-12 diagnostic-shadow 1/1。每次恢复源码，编译失败 / 超时不计有效。
- 真实 `lifecycle_app_probe` 发现并修复退出 hook 在异步阶段重借 App 导致的 RefCell panic：改为 hook 同步阶段释放原生资源，返回 future 不再访问 App。最终真实 `cx.quit()` rc=0，socket 删除、lock 文件保留、进程退出后重新 acquire 成功；绕过 hook 清理会 rc=1。探针独立 F19 / 数据目录 / child，不操作用户实例。
- `lifecycle_probe` 增强验证 `prepare_process_exit` 后进程仍活着时不可创建新 owner；锁由内核在进程死亡时释放。`wake_preview` 直接调用恢复回调，真实隐藏窗口、同 Router、尺寸保持与有效热键状态通过；移除实际 rearm 会明确 FAIL。该测试不等同系统睡眠。
- 原生退出协调：实测 NSTerminateLater 会阻塞 GPUI 调度，改为先返回 NSTerminateCancel、异步排空真实 Router 配置队列、带一次性批准重新 terminate；不覆盖已有 delegate selector、不 fork GPUI。真实退出探针额外读取 config.json，要求新主题 dark 已落盘。
- `verify_termination.py` 注入 700ms 的真实主题写盘前延迟：基线 rc=0 且最终 config.json=dark；跳过退出前等待时 rc=1 且明确主题未落盘，1/1 有效拦截（`/tmp/termination-interception.log`）。延迟与变异均 finally 还原，无测试开关残留。
- 2026-10-04 历史中间结果：新增二进程 GPUI 闭环探针：owner 经生产 lifecycle IPC 被 secondary Forwarded 唤回，visible/key/app_active 与同一 windowNumber/Router 已通过；直接 resume_after_wake 后 hidden 与同一实体/注册值通过。独立 F19 sender 尚未唤回（`/tmp/lifecycle-os-final-1004.log`、`/tmp/lifecycle-os-key-spacing-1004.log` rc=1），已排除 canonical 文本误比较，当时尚未区分 OS 注入与事件消费问题；后续定位及复测见下方最终系统验收，不以注册值代替真实热键生效。
- 边界：真实系统睡眠 / 唤醒事件未执行；不把直接调用恢复函数等同系统睡眠。隐藏慢流完整 0–79 / 同一 Router 在当前 T48 多轮真实 OS 回归均通过。

- 前一轮回归：`/tmp/s071112-final-summary.log` rc=0，210 UI 单测、chat / pages / app / markdown / streaming / settings、settings preferences、shell window 共 8 组预览全部 rc=0 且 PASS。完整 OS 验收不包含在该结果中。

- 2026-10-04 最终系统验收：独立 System Events 的 F19 对照可唤回；CoreGraphics sender 补 Function 位 `1 << 23` 后，`/tmp/lifecycle-f19-function-flag-1004.log` 与源码还原后的 `/tmp/lifecycle-restored-1004.log` 均 rc=0。二进程 IPC、直接 wake rearm 保持隐藏、独立 OS F19 唤回均核对同一 windowNumber/Router 及 visible/key/app_active；最后真实 cx.quit，主题 dark 落盘、socket 清理、lock 保留和重新 acquire 全部通过。`/tmp/lifecycle-os-interception-1004.log` rc=0，ipc-show / f19-function-flag / quit-hook 3/3 有效 FAIL 并还原；`/tmp/termination-final-1004.log` rc=0，真实写盘延迟 700ms 的基线通过，跳过排空队列后明确主题未落盘，1/1 有效拦截。编译/锁屏/权限/命令超时均不计有效拦截。真实系统睡眠/唤醒通知仍未执行，模拟调用不替代该证据。
- 最终 macOS 回归：`/tmp/ui-phase07-final-1004.log` 211/211 UI 单测；`/tmp/phase07-regression-1004.log` 8/8 预览；`/tmp/shell-full-final-1004.log` 完整 T45–T53，均 rc=0 且 PASS。实际浅深主窗口渲染与系统菜单已读取，见 S07-08 / S07-09。此前 F19 注入失败记录保留，最终修正只涉及探针，不改变产品热键逻辑。

- 模块拆分后再次运行 `/tmp/lifecycle-split-final-1004.log` rc=0，保持完整真实 IPC/F19/退出闭环；提交门禁见 `/tmp/phase07-completion-gate-1004.log`。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 跨进程单实例 | v2 新增 flock + ACK IPC | 当前 v1 未安装 single-instance 插件，engine 写入锁仅限进程内；本 spec 明确要求避免两个 v2 同时写数据。不会阻止旧 v1 同时运行，两版并存风险仍保留。 |
| 退出所有权 | 保留 flock 文件描述符至进程死亡，同步清理原生资源，异步排空配置再退出 | 先释放锁会让旧实例残余写入与新 owner 重叠；异步再次借 App 曾导致 RefCell panic；GPUI 默认 200ms 终止上限不足以保障真实设置写盘。 |
| Dock / 激活策略 | 保留默认 Regular | v1 未设置 Accessory / LSUIElement 隐藏 Dock；GPUI 默认 Regular。未根据旧设计隐藏 Dock；不以锁屏下激活失败推断产品缺陷。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |

## 完成记录

- 日期：2026-10-04
- commit：`62c023d`（产品实现）；本轮验收探针与最终证据随本完成记录提交。
- 设计文档处置：剩余 Rust 架构设计已整份退役；why 迁移到对应 spec / 代码，台账与 AGENTS 索引同步。

## 备注

后续 spec 的能力不计入本项完成。

### 发布后缺陷：⌘Q 无法退出（2026-10-05 修复）

- 原因：v1（Tauri 2）在 macOS 自动提供默认应用菜单，⌘Q 来自其中的「退出」；GPUI 不建默认菜单，v2 从未设置。
- 修复：`shell/services.rs` 设置应用菜单「退出 Buddy」并绑定 `cmd-q`，与托盘「退出」共用 `quit_after_save`（等进行中的配置保存写盘后 `cx.quit()`）。
- 证据：开发构建以 HID 事件按 ⌘Q（输入框聚焦）约 250ms 内进程退出；退出后再次启动可正常取得单实例；
  辅助功能读回应用菜单项为「退出 Buddy」、快捷键 Q、修饰键 0（仅 ⌘）。
