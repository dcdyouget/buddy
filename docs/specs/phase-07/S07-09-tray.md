# S07-09 tray 图标与菜单

> 状态: `done`
> Phase: 07
> 依赖: S00-03
> 阻塞: —
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

- [x] 实际菜单点击及浅 / 深图标可见，每项操作可复现。
- [x] 回调有效拦截，系统菜单自动化边界明确。
- [x] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

- 已实现 `shell/tray.rs` 原生托盘与 `shell/services.rs` 主线程事件消费，生产入口装配；原生对象与唯一事件任务随应用存活。左键调用鼠标屏幕定位并呼出，设置复用主 Router，退出等待配置队列。
- `tray_preview --selftest` rc=0（`/tmp/tray-state.log`）：真实 CheckMenuItem 初始、成功读回、忙碌禁用、查询错误禁用、恢复均通过；该证据不等于真实系统菜单点击。
- `verify_services.py` 中 menu-settings / tray-release 有效拦截；menu-readback 首轮暴露探针错误退出码（FAIL 但 rc=0），修复为显式析构原生托盘后按结果退出，定向重测 rc=0、1/1 有效拦截（`/tmp/services-menu-interception-final.log`）。合计 3/3；不是 OS 点击证据。
- 2026-10-04 真实 OS 输入通过（`/tmp/tray-os-debug-1004.log`、`/tmp/tray-os-light-1004.log` 均 rc=0）：左键呼出读回 visible/key/active=true，右键实际菜单点“设置…”进入同一 Router 设置页；真实开关自启后 entry/disk/plist 分别全 true / 全 false，截图复核勾选变化；菜单退出后 parent 校验完整 ACK、删除专用 LaunchAgent 和 sandbox。普通鼠标事件必须显式清除 CGEvent 继承的 Command flags，不能将带修饰键的菜单栏操作当作产品点击失败。
- 原生菜单及模板图标已实际截图读取；`/tmp/tray-light-menu.png` 虽在测试进程 NSApplication.appearance=Aqua 的读回下生成，实际仍为深色，不计为浅色验收。该无效诊断已完全移除，未更改系统外观。随后按真实系统外观补验收：`/tmp/tray-os-system-light-1004.log` rc=0，实际读取 `/tmp/tray-system-light-bar.png`、`/tmp/tray-system-light-menu.png`、`/tmp/tray-system-light-checked.png`，模板图标可见、菜单为浅色、勾选准确。`/tmp/tray-light-system-driver-1004.log` rc=0 证明 finally 恢复原系统深色并读回 true。浅深均为真实 OS 渲染，图片不入库。
- 真实 OS 回调变异：把 SETTINGS_ID 映射改为 Show，重新编译后实际左键呼出成功，再右键点设置；`/tmp/tray-os-mutation-1004.log` probe rc=1，Settings 页/active 组合断言失败，parent 即使收到 child rc=0 仍拒绝 PASS，清理全部成功。源码已 finally 还原；移除全部 appearance / event 临时诊断后重编译，`/tmp/tray-os-restored-1004.log` rc=0，真实全部菜单链路再次通过。包装脚本因预期错误文本与真实文本不同亦 rc=1（`/tmp/tray-os-mutation-driver-1004.log`），有效拦截依据是实际 probe 的 Settings 断言与非零退出码，不以包装脚本错误计数。

- 验证汇总：既有 210 UI 单测与 8 组预览回归均 rc=0（`/tmp/s0712-regression-1004.log`、`/tmp/s0712-window-1004-recheck.log`），新增动画确定性回归后 211/211（`/tmp/ui-phase07-final-1004.log`）。当前重新运行 `/tmp/phase07-regression-1004.log` rc=0，8/8 组各自退出码与 PASS 已校验。全部 OS 菜单链路、浅深实图、原生状态/映射 3/3 与真实 Settings 回调 1/1 有效拦截已记录；当前提交门禁单独记录结果，不把完整 shell 的 T53 失败计为本项通过。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 菜单与左键 | 设置… / 开机自启 / 分隔 / 退出；左键始终显示 | 当前 v1 没有“显示/隐藏”菜单，左键不是 toggle；标题 Buddy、72px template PNG 复用 v1。原目标中的“显示菜单”描述服从当前源码。 |
| 查询失败 | 禁用自启项并显示中文错误 tooltip | 不使用 config 值冒充 OS 查询成功；相较 v1 的配置兜底更严格。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |

## 完成记录

- 日期：2026-10-04
- commit：`0d9903f`（产品）；`a3d8b60`（真实 OS 探针与协议）。
- 提交门禁：`/tmp/phase07-entrance-tray-gate-1004.log` rc=0。
- 设计文档处置：`rust-architecture.md` 仅删 tray 段并登记部分删减；`pages-and-states.md` 最后托盘正文退役后仅剩历史头与空标题，按 RULES §7.2 整份删除，另登记完整退役并移除 AGENTS Document Index。

## 备注

后续 spec 的能力不计入本项完成。
