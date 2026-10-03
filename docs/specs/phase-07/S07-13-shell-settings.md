# S07-13 自启与数据目录设置 UI

> 状态: `done`
> Phase: 07
> 依赖: S07-01, S07-10
> 阻塞: —
> 退役设计文档: —

## 目标

按当前 v1 核查真实入口，不凭旧素材新增 UI；复用设置控件。

## 输入

- `docs/tasks/v2.0.0-gpui/07-shell.md`
- `src/pages/SettingsPage.tsx`
- `src-tauri/src/tray.rs`
- `crates/engine/src/storage.rs`

## 实现要点

- 按当前 v1 核查真实入口，不凭旧素材新增 UI；复用设置控件。
- 自启失败保留旧值；数据目录只定位 / 打开，不迁移历史。

## 验收标准

- [x] 实际入口 / 保存 / 失败恢复有真实事件与系统读回。
- [x] 原生文件管理器打开的自动化证据边界登记。
- [x] 回归、有效拦截、本地实际渲染验收和提交门禁有记录；临时诊断还原。

## 证据

- 当前 `src/pages/SettingsPage.tsx` 只有主题、热键、更新、模型；`src-tauri/src/tray.rs` 中自启唯一入口是托盘原生复选菜单。v2 同样由 `shell/services.rs` 调用 Router 的串行自启事务，复用 S07-10 的 OS 读回 / 保存 / 失败恢复。
- v1 设置页与 tray 都没有“数据目录”用户入口，未发现 Finder / 文件管理器打开命令；本轮不凭旧素材增加该功能。v2 `crates/engine/src/storage.rs::default_data_dir` 沿用 `dirs::data_dir()/com.buddy.chat`，生产入口传给 engine，没有迁移历史。
- 自启注册与真实写盘故障恢复见 S07-10（实现 `0d9903f`）；解锁后 `tray_os_probe` 通过真实托盘点击串联显示、设置、开启自启、写盘失败回滚、关闭自启、退出。`/tmp/tray-rollback-baseline-1004.log` 与 `/tmp/tray-rollback-restored-1004.log` 均 rc=0；专用 LaunchAgent / sandbox 已清理。
- 故障步骤：实际菜单开启成功后，将专用 config.json 暂时换为同名目录，再实际点击关闭自启。ACK 必须为 `config_write=failed set_targets=false,true launch_agent_entry=true disk=true router=true plist=true bytes_equal=true`：真实系统先关闭再恢复开启，原配置字节、磁盘值、Router 与 plist 一致。恢复文件后重新打开真实菜单，`/tmp/tray-rollback-checked-1004.png` 已读取，勾选仍在；之后正常关闭与退出通过。
- 有效拦截：临时跳过 `router_autostart.rs` 的实际系统回滚，真实菜单故障操作得到 `set_targets=[false] launch_agent_entry=false plist_exists=false`，probe rc=1；`/tmp/tray-rollback-mutation-driver-1004.log` rc=0，1/1 有效。源码 finally 还原，恢复后全链路 rc=0。没有用编译错误、权限失败或命令超时计作拦截。
- 原生浅深菜单与图标实际渲染见 S07-09；系统外观已恢复。文件管理器打开没有实际入口，不声称存在自动化或系统验证证据。
- 回归：211/211 UI 单测、8/8 预览与完整 T45–T53 均 rc=0，日志分别为 `/tmp/ui-phase07-final-1004.log`、`/tmp/phase07-regression-1004.log`、`/tmp/shell-full-final-1004.log`。

- 模块拆分后再次运行 `/tmp/tray-split-final-1004.log` rc=0，`/tmp/tray-split-clicks-final-1004.log` 为真实指针操作及逐阶段 ACK；再次读取失败回滚后的原生菜单勾选。提交门禁见 `/tmp/phase07-completion-gate-1004.log`。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 自启 UI | 保留 tray 入口，不新增设置页开关 | 当前 v1 没有设置页自启控件；复用原生 menu 与配置队列即可。 |
| 数据目录 | 沿用兼容路径，不新增打开入口 | 当前 v1 无此 UI 或文件管理器命令；旧素材的入口描述不构成新增功能授权。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |

## 完成记录

- 日期：2026-10-04
- commit：`0d9903f`（产品实现）；本轮验收探针与最终证据随本完成记录提交。
- 设计文档处置：无负责退役文档；当前 v1 入口核查与未新增数据目录 UI 的理由已登记。

## 备注

后续 spec 的能力不计入本项完成。
