# S07-10 开机自启

> 状态: `done`
> Phase: 07
> 依赖: S00-03
> 阻塞: —
> 退役设计文档: —

## 目标

复用 Spike 系统登录注册 / 查询方案，以实际 bundle / 可执行路径注册。

## 输入

- `docs/tasks/v2.0.0-gpui/07-shell.md`
- `src-tauri/src/lib.rs`
- `src-tauri/src/tray.rs`
- `docs/specs/phase-00/S00-03-hotkey-tray-autostart.md`

## 实现要点

- 复用 Spike 系统登录注册 / 查询方案，以实际 bundle / 可执行路径注册。
- 失败不误报成功，验证保存并恢复原有自启状态。

## 验收标准

- [x] 系统查询 / 开启 / 关闭 / 失败恢复有读回和有效拦截。
- [x] 开发与正式 bundle 路径差异登记。
- [x] 回归、有效拦截与提交门禁有记录；本项为无独立界面的 OS 服务，托盘实际渲染和 OS 点击由 S07-09 验收；临时探针与沙盒已清理。

## 证据

- `SystemAutostart` 使用 auto-launch 0.6 LaunchAgent 模式与绝对 current_exe；`router_autostart.rs` 复用配置队列，前项完成后取基底，OS 切换并读回后保存 auto_start，写盘失败恢复原 OS 状态，不发布失败配置。
- `cargo run -q -p buddy-app --example autostart_probe` rc=0（`/tmp/autostart-os.log`）：专用 `BuddyS0710Probe-44886` 初始关闭，开启后读回 true，plutil 解析 ProgramArguments 得到真实 probe 路径，关闭后读回 false 且 plist 删除，最终清理通过。未触碰真实 Buddy 登录项。
- `cargo test -q -p buddy-ui --lib` 205/205，rc=0（`/tmp/s070910-ui.log`），含 OS 操作 / 读回 / 回滚失败分支。
- `autostart_preferences_preview --selftest`：两次自启 toggle 中插入真实主题保存事件，最终主题和其他字段保留；fake OS 失败不写盘。额外使用专用真实 LaunchAgent，确认开启读回 true 后，故意把沙盒 config.json 换成目录导致写盘失败，OS 恢复 false，内存 / 磁盘 / 原始字节保持旧值，专用 plist 与故障沙盒清理。
- `verify_services.py` 的 os-readback / os-rollback / native-registration / config-rollback / config-field 共 5/5 有效拦截。增强真实 OS 回滚后又定向复测 config-rollback / config-field，rc=0、2/2（`/tmp/services-native-config-interception.log`，对应临时目录含基线与故意失败日志）。编译失败不计拦截。
- 证据边界：auto-launch 的 LaunchAgent 查询检查注册 plist，不证明已经实际注销再登录启动。开发路径为 target/debug 下可执行文件；正式包应为 Buddy.app/Contents/MacOS 下实际 current_exe，本轮未构建正式 bundle。托盘真实点击归 S07-09。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 行为依据 | 当前 v1 源码 | 旧素材尺寸 / 毛玻璃 / 置顶描述不覆盖当前源码与已接受决定。 |
| 写盘失败恢复 | 回滚 OS 状态并读回，不发布新配置 | v1 保存失败仅记日志，会留下系统与配置分离；此偏离单独登记。 |
| 同一注册名称 | 产品沿用 Buddy，探针使用唯一专用名称 | 开发产品与正式包的 Buddy.plist 共用名称；只在用户操作时切换，不在启动时静默覆盖旧注册。自测不操作正式名称。 |
| 视觉验收 | agent 本地读取真实渲染 | 用户已授权自行验收，不再等待用户目检。 |

## 完成记录

- 日期：2026-10-03
- commit：`0d9903f`
- 设计文档处置：本项无退役归属。
- 最终验证：205 UI 单测、8 组预览回归 rc=0（`/tmp/s070910-final-summary.log`）；提交门禁 rc=0（`/tmp/gate-s070910.log`）。专用 LaunchAgent 无残留。服务完成不代表 S07-09 的真实菜单验收完成。

## 备注

后续 spec 的能力不计入本项完成。
