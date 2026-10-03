# S07-11 单实例与生命周期（无窗口存活 / 休眠唤醒）

> 状态: `doing`
> Phase: 07
> 依赖: S07-01
> 阻塞: —（桌面已解锁，补已有实例聚焦与重绑后 OS 热键；真实睡眠证据单列）
> 退役设计文档: docs/design/rust-architecture.md 生命周期 / 剩余职责与依赖

## 目标

第二实例向已有实例发送唤起，不并行写数据；核查 v1 激活策略 / Dock。

## 输入

- `docs/tasks/v2.0.0-gpui/07-shell.md`
- `src-tauri/src/lib.rs`
- `src-tauri/src/window/mod.rs`
- `docs/design/rust-architecture.md`

## 实现要点

- 第二实例向已有实例发送唤起，不并行写数据；核查 v1 激活策略 / Dock。
- 无可见窗口仍保持 engine / 注册；退出清理资源；唤醒复核热键和定位。

## 验收标准

- [ ] 双进程、隐藏继续流式、退出资源释放可复现并有效拦截。
- [ ] 真实休眠唤醒缺口如实记录。
- [ ] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

- 产品入口先获取用户私有目录中的 flock owner，再创建 engine / GPUI 窗口 / 热键 / 托盘。第二进程通过 Unix socket 发送 Wake，收到 ACK 才退出；仅持有 flock 的 owner 可清理 stale socket，锁文件保留避免 inode 竞态。
- `lifecycle_probe` 首轮 rc=0（`/tmp/lifecycle-probe.log`）：专用 child READY 后启动第二进程，Forwarded + owner Wake ACK；只 kill 专用 owner 模拟崩溃，stale socket 恢复；正常 Drop 清理 socket、保留锁文件并可再获取。探针目录与专用子进程清理。
- `on_system_wake` 重新注册同一热键并清理 Pressed 状态，注册失败不能以旧 current 冒充有效；重新读取屏幕工作区并裁剪原生位置，不调用 show、不改变显隐。托盘退出在调用 quit 前等配置队列；`on_app_quit` 同步注销热键和外点监听 / tray，原生退出使用 Cancel → 排空配置队列 → Now 重试，避免 GPUI 200ms shutdown 上限截断设置；IPC 清理后锁保留到内核终止进程，避免旧进程残余写入与新 owner 重叠。
- `verify_lifecycle.py` 最终 rc=0（`/tmp/lifecycle-interception-final.log`）：本项 owner-lock / stale-socket / socket-cleanup / exit-lock / rearm-unregister / rearm-pressed / wake-registration / quit-hook 共 8/8 有效拦截；另含 S07-12 diagnostic-shadow 1/1。每次恢复源码，编译失败 / 超时不计有效。
- 真实 `lifecycle_app_probe` 发现并修复退出 hook 在异步阶段重借 App 导致的 RefCell panic：改为 hook 同步阶段释放原生资源，返回 future 不再访问 App。最终真实 `cx.quit()` rc=0，socket 删除、lock 文件保留、进程退出后重新 acquire 成功；绕过 hook 清理会 rc=1。探针独立 F19 / 数据目录 / child，不操作用户实例。
- `lifecycle_probe` 增强验证 `prepare_process_exit` 后进程仍活着时不可创建新 owner；锁由内核在进程死亡时释放。`wake_preview` 直接调用恢复回调，真实隐藏窗口、同 Router、尺寸保持与有效热键状态通过；移除实际 rearm 会明确 FAIL。该测试不等同系统睡眠。
- 原生退出协调：实测 NSTerminateLater 会阻塞 GPUI 调度，改为先返回 NSTerminateCancel、异步排空真实 Router 配置队列、带一次性批准重新 terminate；不覆盖已有 delegate selector、不 fork GPUI。真实退出探针额外读取 config.json，要求新主题 dark 已落盘。
- `verify_termination.py` 注入 700ms 的真实主题写盘前延迟：基线 rc=0 且最终 config.json=dark；跳过退出前等待时 rc=1 且明确主题未落盘，1/1 有效拦截（`/tmp/termination-interception.log`）。延迟与变异均 finally 还原，无测试开关残留。
- 2026-10-04 新增二进程 GPUI 闭环探针：owner 经生产 lifecycle IPC 被 secondary Forwarded 唤回，visible/key/app_active 与同一 windowNumber/Router 已通过；直接 resume_after_wake 后 hidden 与同一实体/注册值通过。独立 F19 sender 尚未唤回（`/tmp/lifecycle-os-final-1004.log`、`/tmp/lifecycle-os-key-spacing-1004.log` rc=1），已排除 canonical 文本误比较，正在区分 OS 注入与事件消费问题，不以注册值代替真实热键生效。
- 边界：真实系统睡眠 / 唤醒事件未执行；不把直接调用恢复函数等同系统睡眠。隐藏慢流完整 0–79 / 同一 Router 在当前 T48 多轮真实 OS 回归均通过。

- 最终回归：`/tmp/s071112-final-summary.log` rc=0，210 UI 单测、chat / pages / app / markdown / streaming / settings、settings preferences、shell window 共 8 组预览全部 rc=0 且 PASS。完整 OS 验收不包含在该结果中。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 跨进程单实例 | v2 新增 flock + ACK IPC | 当前 v1 未安装 single-instance 插件，engine 写入锁仅限进程内；本 spec 明确要求避免两个 v2 同时写数据。不会阻止旧 v1 同时运行，两版并存风险仍保留。 |
| Dock / 激活策略 | 保留默认 Regular | v1 未设置 Accessory / LSUIElement 隐藏 Dock；GPUI 默认 Regular。未根据旧设计隐藏 Dock；不以锁屏下激活失败推断产品缺陷。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：

## 备注

后续 spec 的能力不计入本项完成。
