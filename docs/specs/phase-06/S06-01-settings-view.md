# S06-01 设置页骨架与控件集

> 状态: `done`
> Phase: 06
> 依赖: S03-01
> 阻塞: —
> 退役设计文档: `docs/design/pages-and-states.md` SettingsPage 的骨架 / overlay 部分；`docs/design/component-mapping.md` 的 SlideInPanel / 共用控件部分（其余设置能力保留至对应 spec）

## 目标

将设置作为保留底层页面的全尺寸覆盖层，提供分组、滚动、键盘导航与共用表单控件。

## 输入

- `src/pages/SettingsPage.tsx`
- `src/components/shared/SlideInPanel.tsx`
- `src/styles/global.css`
- `crates/ui/src/chat/router.rs`

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S06-01-1 | 实现约束 | 返回按钮与滚动内容层按 v1 信息层级实现；底层 Composer / Conversation 实体保留。 |
| S06-01-2 | 实现约束 | 共用设置分组、按钮、开关、单行输入与下拉控件仅使用 Theme 令牌。 |
| S06-01-3 | 实现约束 | 侧滑覆盖层进入 / 退出遵循 v1 200ms 与减弱动态效果；退出时立即释放输入拦截。 |
| S06-01-4 | 实现约束 | 替换 Router settings_placeholder；不在 UI 代码改变窗口尺寸。 |
| S06-01-5 | 实现约束 | 更新 / 自启 / 全局热键注册与窗口壳分别移交 S08-11 / S07-13 / Phase 07，本 spec 不伪造后端。 |

## 验收标准

- [x] 真实点击设置 / 返回与滚动、Tab 焦点导航自测通过，往返保持草稿、流式任务及窗口尺寸。
- [x] 共用控件输入 / 点击与侧滑显隐自测通过，逐项实施拦截验证并记录未覆盖项。
- [x] 读取实际 GPUI 渲染核对浅 / 深主题及设置覆盖层，记录与 v1 的差异。

## 证据

| 项 | 证据 |
|----|------|
| Router 真实交互 | `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example settings_preview -- --selftest`：T35 PASS。以 `window.dispatch_event` 点击底层设置入口、滚动、Tab / Shift+Tab / Enter、返回；每步 `window.refresh(); window.draw(cx).clear(cx)`。120 个配置模型使设置可滚动，30 条历史使底层 Transcript 可滚动；设置 offset 0→-420px，底层列表位置保持。 |
| 会话与尺寸 | 同一 T35：进入帧实际 x=761px、稳定帧 x=1px，内容 758×638px，窗口始终 760×640px；关闭立即非交互，退出动画仍绘制且底层入口可立即点击。草稿 / 历史不丢失；设置期间注入 StreamEvent 后文本继续增长。此项使用真实 Router / Conversation 的事件夹具，不声称联网 engine 验收。 |
| 共用控件 | `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example settings_preview -- --selftest-controls`：T36 PASS。真实 Cmd+V 将带换行剪贴板文本归一为单行，真实点击开关 / 下拉选项 / 按钮，实际焦点的 Tab / Shift+Tab、方向键和 Esc 均通过；无直接业务方法替代鼠标结果。追加 `--dark` 也通过。 |
| 动画逻辑 | `cargo test -q -p buddy-ui --lib settings::panel::tests::exits_release_input_before_paint_finishes`：起点 / 200ms 终点与退出立即释放输入；动画位移另由 T35 实际布局证明。 |
| 本地视觉读取 | 2026-10-01 临时启用 GPUI test-support，读取设置浅 / 深 760×640 与共用控件 560×640 的实际 `window.render_to_image()` 输出，核对标题、分组、间距、滚动区域、菜单覆盖及深色图标。诊断代码与 Cargo.toml / Cargo.lock 全部还原，图片不入库。视觉样式与系统减弱动态效果分支无独立自动化证据，不将交互拦截等同视觉测试。 |
| 拦截验证 | `python3 scripts/settings/verify_phase06.py`；14/14 有效拦截：动画单测、底层滚轮隔离、设置挂载、鼠标 / 键盘返回、设置滚动、覆盖层 occlude、退出释放、动画位移、草稿保留、下拉鼠标 / 键盘 / Esc、单行粘贴。键盘变异初版产生借用编译错误未计入，修正为可编译变异后 T36 明确 FAIL。每次只变异一处，检查明确 FAIL 与非零退出，编译错误 / 超时不计为拦截；结束还原原始字节。 |
| 回归与门禁 | `cargo test -q -p buddy-ui --lib`：120 passed。`chat_preview`、`pages_preview`、`app_preview`、`markdown_preview`、`streaming_preview` 均按 `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example <名字> -- --selftest` 全量执行，rc=0 且有 PASS（含搜索 / 生图 / 附件 T31–T34）。提交门禁 `scripts/gate.sh`：rc=0，纪律 18 项与其拦截、图标、v2 全目标和 v1 编译全部通过。 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 设置叠加层 | Router 保留底层 Conversation / Composer 和一个复用的 SettingsView | 与 v1 一致：设置切页不销毁草稿与进行中的回复，窗口尺寸仅交给应用外壳。 |
| 退出交互 | 关闭即将 SettingsView 置为非交互，SlideMotion 保留 200ms 退出绘制 | 迁移 v1 SlideInPanel 的退出立即释放鼠标语义，避免透明层拦截下一次底层点击；入口 / 出口使用 EASE_STANDARD，减弱动态效果时立即完成。 |
| 子项边界 | 骨架只读显示当前外观、快捷键、模型；共用控件在独立示例中验证 | S06-02/03/05/06 各自承接编辑流程；当前不提供尚无保存行为的产品按钮，也不显示内部 spec 编号。 |
| Transcript 滚轮隔离 | 捕获阶段也检查实际 hitbox 的 `should_handle_scroll` | 原先只比较边界，会绕过设置覆盖层的 occlude，吞掉前景滚轮；T35 同时验证前景滚动和底层列表静止。 |
| 菜单边界记录 | canvas 写入持续保存的 Cell，外部点击读取最近实际绘制边界 | 逐帧临时记录在未绘制回调中会被清空，真实选项点击被误判为外部点击；T36 的鼠标选项选择独立覆盖此错误。 |
| 共用控件外观差异 | 开关为 40×24px GPUI 自绘控件，下拉为自绘菜单；当前仅用于控件示例 | v1 模型表使用 HTML 原生 checkbox / select，GPUI 没有对应 HTML 控件，故共用控件的外观有差异；设置产品页在 S06-02/03/05/06 接入编辑时仍以对应 v1 子组件为准。 |
| 后续子项输入契约 | 可交互子组件必须接收 SettingsView 的 active 状态，退出期间只绘制 | 骨架当前只有返回与滚动交互，关闭时已取消其处理器；未来内嵌表单不可在 200ms 退出帧继续响应。 |
| MCP 与热键范围 | MCP 的 v1 仅有数据结构；热键系统注册归 Phase 07 | v1 SettingsPage 中无 MCP 编辑器，后续 S06-04 须明确新增差异；本 spec 不声称存在 MCP 连接或热键注册效果。 |

## 完成记录

- 日期：2026-10-01
- commit：`8a8c150`
- 设计文档处置：按 RULES §7 部分删除 SettingsPage overlay 骨架与组件映射的外层设置组合 / SlideInPanel 角色；why 已迁入上述决策记录与代码注释，登记于 `docs/specs/design-deletions.md` 的「已部分删减的文档」。设置子项、添加 Provider 与窗口外壳段落保留。
