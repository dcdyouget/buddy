# S06-06 主题设置界面

> 状态: `done`
> Phase: 06
> 依赖: S06-01, S03-06
> 阻塞: —
> 退役设计文档: `docs/design/component-mapping.md` / `docs/design/pages-and-states.md`（主题及设置保存段落，部分退役）

## 目标

移植仅浅 / 深两种外观选择，立即应用并持久化后在重启恢复。

## 输入

- `src/components/settings/ThemeSetting.tsx`
- `crates/ui/src/theme_system/mod.rs`
- `crates/engine/src/models/config.rs`

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S06-06-1 | 实现约束 | 用户已决定不跟随系统主题，任务素材中的跟随系统不实现。 |
| S06-06-2 | 实现约束 | 切换使用 Theme 全局安装 API、保存到真实 engine 配置并同步路由。 |

## 验收标准

- [x] 真实点击切换即时生效，重启 / 读盘恢复且窗口尺寸不变。
- [x] 禁用应用 / 保存的变异必须被自测拦截。
- [x] 本地读取实际浅 / 深设置页帧，与 v1 分段控件对照。

## 证据

| 项 | 证据 |
|----|------|
| 真实操作 | `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example settings_preview -- --selftest-preferences`：T44 通过真实点击、ShiftTab、Enter / Space 核对保存后同次操作换色；控件选择 / Router / 全局 Theme / 磁盘 JSON 一致；目录故障保留深色与错误提示，可重试浅色。各输入都经过 `window.dispatch_event`，每帧 `refresh` / `draw.clear`。 |
| 从磁盘恢复 | T44 使用新 engine `get_config`；先把全局主题重置为浅色，再用读到的配置创建全新 PageRouter，核对深色恢复、控件选中与实际 viewport 尺寸保持。此证据覆盖新 Router 初始化，不宣称 Phase 07 产品进程重启链路已实现。 |
| 保存基底 | `NO_PROXY=127.0.0.1,localhost cargo test -q -p buddy-ui --lib chat::router::preferences`：字段增量测试核对热键与主题连续修改及其他配置完整保留；写入先等待上一任务，之后再读取成功基底。 |
| 拦截 | `python3 scripts/settings/verify_preferences.py` 首批 16/16 通过，其中主题字段、点击事件、全局应用、创建 Router 恢复、真实保存与错误提示变异均命中行为 FAIL；编译错误不计，原字节全部还原。 |
| 追加拦截与焦点 | 扩充 T40 核对新增层覆盖 / 返回时模型、主题和热键的焦点入口全部释放 / 恢复；`python3 scripts/settings/verify_preferences.py --case theme-error-chinese-label --case preferences-input-release` 2/2 有效拦截，中文错误标签和覆盖层禁用保护均能抓到。连同 Enter 启动的追加 1 项，共 19 项有效拦截。 |
| 回归 / 门禁 | 161 个 UI 单测；chat / pages / app / markdown / streaming 五个 `--selftest`、设置 T35 / T36 / T39 / T41 / T42 与扩充 T40、T43 / T44 均 rc=0 且含 PASS；`scripts/gate.sh` rc=0，编译无 warning。图片和销毁窗口错误日志是现有 T32 / T34 的故障注入。 |
| 本地渲染 | 按 handoff §6.5 第 27 项读取浅 / 深及 560 / 760 实际帧，核对源码中的 Sun / Moon、分段选中态、键盘聚焦与保存错误态；图标经 `scripts/icons/lucide_svg.py` 从 v1 图形数据生成。临时诊断代码 / `test-support` / 锁文件变化已还原，无截图入库。 |
| 证据边界 | 像素样式、系统阴影、销毁窗口后的保存、真实阻塞请求的多入口队列交错，该项无独立自动化证据；失败后 Tab 滚动只保证按钮可见，不独立验证错误文案也被自动滚入视口。 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 应用时机 | 沿用当前 v1 `configStore.updateTheme` 的先保存后发布；成功后 `set_appearance` 刷新全部窗口，失败保留旧外观 | “立即应用”指这次操作保存成功后直接换色，不需要再次进入设置；不提前展示无法落盘的主题 |
| 只提供浅 / 深 | 控件仅发出 `Theme::Light` / `Theme::Dark`，没有跟随系统选项 | 用户已决定不跟随系统主题，配置契约和 v1 同样只有两种 |
| 恢复入口 | Router 创建时从已读入的 `AppConfig.theme` 安装全局主题；后续成功配置发布同步控件与全局主题 | 真实 engine 读盘结果必须驱动新窗口第一帧，而不是依赖预览启动参数 |
| 控件交互差异 | 保存期间短暂禁用选择；新增就地错误、键盘聚焦品牌色提示 | 当前 v1 未做控件级禁用 / 错误提示；选择、图标、间距与浅 / 深表面继续按源码，键盘反馈保证原生控件可访问 |

## 完成记录

- 日期：2026-10-02
- commit：`3bdba13`
- 设计文档处置：主题 / 设置保存段落与旧 `html.dark` 说明已部分删除；登记 `docs/specs/design-deletions.md`，窗口与系统菜单栏内容留给 Phase 07。
