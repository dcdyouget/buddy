# S07-09 tray 图标与菜单

> 状态: `doing`
> Phase: 07
> 依赖: S00-03
> 阻塞: —（桌面已解锁，补真实 OS 菜单链路验收）
> 退役设计文档: docs/design/pages-and-states.md 系统托盘入口；docs/design/rust-architecture.md tray 职责

## 目标

中文显示 / 设置 / 自启 / 退出菜单与图标资源，接统一窗口控制。

## 输入

- `docs/tasks/v2.0.0-gpui/07-shell.md`
- `src-tauri/src/tray.rs`
- `docs/specs/phase-00/S00-03-hotkey-tray-autostart.md`

## 实现要点

- 中文显示 / 设置 / 自启 / 退出菜单与图标资源，接统一窗口控制。
- 托盘与 GPUI 主线程事件循环共存，退出清理而普通隐藏不退出。

## 验收标准

- [ ] 实际菜单点击及浅 / 深图标可见，每项操作可复现。
- [ ] 回调有效拦截，系统菜单自动化边界明确。
- [ ] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

- 已实现 `shell/tray.rs` 原生托盘与 `shell/services.rs` 主线程事件消费，生产入口装配；原生对象与唯一事件任务随应用存活。左键调用鼠标屏幕定位并呼出，设置复用主 Router，退出等待配置队列。
- `tray_preview --selftest` rc=0（`/tmp/tray-state.log`）：真实 CheckMenuItem 初始、成功读回、忙碌禁用、查询错误禁用、恢复均通过；该证据不等于真实系统菜单点击。
- `verify_services.py` 中 menu-settings / tray-release 有效拦截；menu-readback 首轮暴露探针错误退出码（FAIL 但 rc=0），修复为显式析构原生托盘后按结果退出，定向重测 rc=0、1/1 有效拦截（`/tmp/services-menu-interception-final.log`）。合计 3/3；不是 OS 点击证据。
- 待解锁后检查浅 / 深菜单栏图标、真实左 / 右键和逐项菜单动作；生产 services 消费链尚无真实菜单回调验收，不标 done。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 菜单与左键 | 设置… / 开机自启 / 分隔 / 退出；左键始终显示 | 当前 v1 没有“显示/隐藏”菜单，左键不是 toggle；标题 Buddy、72px template PNG 复用 v1。原目标中的“显示菜单”描述服从当前源码。 |
| 查询失败 | 禁用自启项并显示中文错误 tooltip | 不使用 config 值冒充 OS 查询成功；相较 v1 的配置兜底更严格。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：

## 备注

后续 spec 的能力不计入本项完成。
