# S06-05 热键录制交互

> 状态: `doing`
> Phase: 06
> 依赖: S06-01, S00-03
> 阻塞: —
> 退役设计文档: `docs/design/component-mapping.md` / `docs/design/pages-and-states.md`（热键及设置保存段落，部分退役）

## 目标

移植快捷键显示、录制、组合键校验、取消与配置保存，向 Phase 07 交付注册输入。

## 输入

- `src/components/settings/HotkeyRecorder.tsx`
- `src/components/settings/HotkeySetting.tsx`
- `docs/evidence/s00-03/shell-integration.rs`

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S06-05-1 | 实现约束 | 复现 v1 键名与平台修饰键格式；录制态捕获键盘，纯修饰键不提交。 |
| S06-05-2 | 实现约束 | 全局注册、系统冲突与生效能力属于 Phase 07；UI 不声称已注册。 |
| S06-05-3 | 实现约束 | 保存错误保留旧配置与可读提示；Esc 取消录制不能意外关闭底层任务。 |

## 验收标准

- [x] 真实键盘覆盖组合、裸键、纯修饰键、Esc 取消及保存，核对明文配置往返。
- [x] 对应拦截验证及本地渲染检查完成；Phase 07 注册交接明确。

## 证据

| 项 | 证据 |
|----|------|
| 纯状态 | `NO_PROXY=127.0.0.1,localhost cargo test -q -p buddy-ui --lib settings::hotkey::state`：7 个测试覆盖主键释放才提交、裸键 / Fn 拒绝、纯修饰键清空、Esc 取消、规范键名与平台键帽、修饰键先释放仍保留快照。 |
| 真实输入与磁盘 | `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example settings_preview -- --selftest-preferences`：T43 通过真实 `window.dispatch_event` 鼠标、KeyDown / KeyUp / ModifiersChanged 覆盖裸键、纯修饰键释放、Esc 保留设置页、Enter 启动、Cmd+Tab、Cmd+Shift+K 快照、平台键帽、写盘失败中文提示和 Cmd+Alt+P 重试；使用独立 engine `get_config` 核对 JSON，强制 `refresh` / `draw.clear` 出帧。 |
| 拦截 | `python3 scripts/settings/verify_preferences.py`：首批 16 项已有效拦截（单测 rc=101 / GUI rc=1，必须含行为失败标记；编译错误不计）；追加 `--case hotkey-keyboard-start` 1/1 有效拦截。热键侧覆盖 Esc、裸键 / Fn、修饰键快照、纯修饰键、大小写、平台键帽、字段写入、候选事件、Tab 分派、错误提示和 Enter 启动；每项均还原原始字节。 |
| 覆盖层释放 / 追加拦截 | 扩充 T40：真实新增层返回操作核对模型、主题、热键三个控件的焦点入口同时释放 / 恢复。`python3 scripts/settings/verify_preferences.py --case theme-error-chinese-label --case preferences-input-release`：2/2 有效拦截；与前 16+1 项合计 19 项，最终脚本可完整复现。 |
| 回归 / 门禁 | 161 个 UI 单测；chat / pages / app / markdown / streaming 五个 `--selftest`、设置 T35 / T36 / T39 / T41 / T42 以及扩充 T40、T43 / T44 均 rc=0 且含 PASS；`scripts/gate.sh` rc=0。预览中的缺失图片及已销毁窗口日志来自 T32 / T34 故障注入，恢复与保存持续性断言均通过。 |
| 本地渲染 | 按 handoff §6.5 第 26 项本地读取浅色 760×640、深色录制态 760×640、深色 560×640、浅色双保存错误 560×640 实际帧；键帽、录制文案和错误未溢出。临时 `test-support` / 锁文件 / 诊断代码已还原，未提交图片。 |
| 证据边界 | 系统注册 / 冲突属于 S07-03；像素细节、系统阴影、销毁窗口后的保存持续性、真实请求阻塞期间的队列交错，该项无独立自动化证据。字段增量保留有纯测试，逐次等待的真实写盘不等于并发交错验证。 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 配置发布 | 候选热键复用 Router 配置队列，写盘成功才更新键帽；失败保留旧值并显示中文错误 | 当前 v1 `configStore.updateHotkey` 已是先保存后发布，避免界面声称尚未保存的热键生效 |
| 修饰键与事件 | Cmd / Ctrl 归一为 `CmdOrCtrl`；Shift / Alt 按 v1 顺序；在主键 KeyDown 捕获修饰键，KeyUp 才提交；Fn 单独不算有效修饰键 | GPUI 平台输入对应浏览器 capture 监听；修饰键先释放不能丢失组合，裸键和纯修饰键继续等待 |
| 系统能力边界 | 仅修改 `AppConfig.hotkey`；不注册系统快捷键，不检测系统占用 | 全局唤起与冲突处理由 S07-03 接入；保存成功表示配置已写盘 |
| 控件交互差异 | 保存期间暂时禁用重新录制；新增保存错误文案 | v1 没有控件级保存禁用与就地错误；避免重复提交并使失败可见，外观以 CSS 覆盖后的 v1 样式为准 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
