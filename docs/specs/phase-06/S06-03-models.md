# S06-03 模型列表与上下文配置

> 状态: `done`
> Phase: 06
> 依赖: S06-02
> 阻塞: —
> 退役设计文档: `docs/design/component-mapping.md` 的 ModelList / ModelRow / StatusDot 部分；`docs/design/pages-and-states.md` 的剩余模型列表说明（其他设置与窗口段落保留）

## 目标

移植模型列表的默认项、启用及上下文与能力配置，以当前 v1 已接入界面为准。

## 输入

- `src/components/settings/ModelList.tsx`
- `src/components/settings/ModelRow.tsx`
- `src/stores/configStore.ts`
- `crates/engine/src/models/config.rs`

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S06-03-1 | 实现约束 | 模型标识必须使用 engine 的真实 ID，不按分隔符猜 Provider。 |
| S06-03-2 | 实现约束 | 上下文、能力沿用 v1 实际行为；配置变更与 Composer 模型菜单同步。当前 v1 无删除入口，不新增删除确认。 |
| S06-03-3 | 实现约束 | 与 Provider 添加、Composer 模型选择复用配置串行保存，写盘成功才发布；失败留在设置页并保留原配置。 |

## 验收标准

- [x] 真实点击 / 输入覆盖默认模型、开关与上下文 / 能力更新并核对磁盘；核对删除入口在当前 v1 不适用。
- [x] 相应拦截验证通过，未覆盖边界如实记录。
- [x] 本地读取实际浅 / 深主题渲染并记录偏差。

## 证据

| 项 | 证据 |
|----|------|
| 配置单测 | `cargo test -q -p buddy-ui --lib settings::model_config_tests`：10 项通过；完整作用域与含 `::` 的 Provider ID、同名模型隔离、停用非默认保持当前默认 / 默认回退 / 全停用 / 重新启用、精确 32768、其他配置字段保留、未知 ID 不变、协议能力限制。 |
| 保存基底单测 | `cargo test -q -p buddy-ui --lib chat::router::config_save::tests`：3 项通过；能力修改 / Provider 添加后保留后继菜单选择，更早完成不覆盖较晚即时选择。该项证明纯状态合并，不等同于真实异步队列阻塞顺序。 |
| 延迟阈值单测 | `cargo test -q -p buddy-ui --lib settings::model_list`：浅 / 深主题 499、500、1499、1500ms 对应 success / warning / error 令牌。 |
| 真实事件 T41 / T42 | `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example settings_preview -- --selftest-models` 返回 0，T41 / T42 PASS。用 `window.dispatch_event` 输入，`window.refresh(); window.draw(cx).clear(cx)` 强制出帧；核对默认 / 启用 / 精确上下文 / Vision / OpenAI 生图 / Anthropic 禁用生图、跨 Provider 同名隔离、Tab / ShiftTab / Enter / Down、重开一致及真实 engine 磁盘 JSON；新增层打开后底层模型焦点释放、退出恢复，添加入口从视口外经 ShiftTab 自动滚入并 Enter 重开（T40 扩充）。故意把 `config.json` 变为目录，验证失败不发布、显示错误、下拉恢复旧值、原磁盘备份不变，恢复后同一操作重试成功。 |
| 本地读取实际渲染 | 2026-10-02 临时启用 GPUI `test-support`，在 resize 回调稳定后断言实际 viewport 为 760×640 / 560×640；读取浅 / 深四个 `render_to_image()` 实际帧，核对默认徽章、状态点、禁用透明度、上下文位于名称下、能力控件和添加按钮同一卡片，560 宽度控件可见。诊断代码与 Cargo.toml / Cargo.lock 已按备份字节还原，不入库。 |
| 拦截验证 | `python3 scripts/settings/verify_models.py` 覆盖 22 项：默认 / 回退 / Provider 作用域 / 精确选项及实际写入 / Vision / 生图协议、两处延迟阈值、成功配置基底 / 后继菜单选择 / 基底读取、真实鼠标与默认键盘入口、动态上下文选项、失败错误 / 受控值恢复、Tab 导航 / 添加入口自动滚动、成功发布及新增层释放模型输入。首轮 21 项有效；上下文 reset 的初始变异导致编译错误，未计入有效拦截，修正为可编译的 no-op 后 `--case model-context-reset` 行为 FAIL，22 项均已有效拦截并还原。 |
| 受影响旧拦截 | `python3 scripts/settings/verify_phase06.py --case provider-save-failure --case provider-return-conversation --case provider-publish-on-failure --case provider-context-exact --case keyboard-back --case exit-input-release --case select-mouse --case select-keyboard`：8/8 行为 FAIL 被有效拦截并还原，保护 Provider 成功 / 失败发布、精确选项和旧设置交互。 |
| 回归 | 149 项 UI 单测通过；设置骨架 / 共用控件 / Provider 自测以及 `chat_preview`、`pages_preview`、`app_preview`、`markdown_preview`、`streaming_preview` 五个 `--selftest` 返回 0 且输出 PASS。app 的故意缺图重试场景仍会打印预期的资产加载失败，T34 验证重试成功。 |
| 提交门禁 | `scripts/gate.sh > /tmp/gate.log 2>&1` 返回 0（无外接管道），涵盖纪律 / 门禁反证 / workspace 与 v1 编译。仅本地提交，v1 源码和临时 test-support / 锁文件未改。 |
| 非自动化边界 | 样式像素、系统阴影、销毁窗口后的保存持续性，以及真实请求阻塞期间 Provider / 模型编辑 / 菜单选择交错，该项无独立自动化证据；T42 后半是逐次等待的配置往返，不据此宣称并发排队被拦截。 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 删除范围核对 | 当前不新增模型 / Provider 删除入口，原「删除确认」验收不适用 | 当前 ModelList / ModelRow / SettingsPage 没有删除动作；configStore 仅有未接 UI 的 removeProvider。按当前运行代码迁移，不把过时描述当成已有行为。 |
| 配置写入基底 | 所有操作等待上一任务后读取独立的最后成功配置；仅持久化成功才推进基底 | 原菜单保存捕获提前快照，与 Provider / 能力修改交错会丢字段。保存任务持有共享基底及队列，即使窗口实体销毁也不会依赖陈旧回退快照。 |
| 菜单即时反馈 | 保留 S05-15 已验收的立即选中，较早保存完成时覆盖层仍显示更晚的菜单选择；写盘按操作顺序 | 当前 v1 configStore 已是先写盘后发布，S05-15 关于 v1 先 set 的旧描述已不符合当前源码。本次保留既有即时交互，独立记录乐观选择与已写盘配置，不再把它混入后继快照。 |
| 模型保存反馈（偏离 v1） | 写盘期间短暂禁用列表控件，失败显示设置内错误，保留原配置再操作重试 | 避免用户继续操作尚未发布的状态；v1 loading 没有禁用模型行。本次写盘成功后更新列表、Composer 能力和模型菜单数据。 |
| 自绘选择控件（偏离 v1） | 沿用 S06-01 自绘 checkbox/select；上下文行内 selector 高 32px，v1 使用更紧凑的原生 select | 复用已有主题 / 输入 / viewport 翻转菜单；实际四帧已读取，结构及行为按 v1，原生控件像素不完全一致。 |
| 上下文单一实现 | 新增流程与已保存模型共用精确 options / 标签函数；外部同步下拉选项不发用户编辑事件 | 33K 只用于显示，配置仍为 32768；选项随当前值变化时必须同步索引和值，异步订阅不能用临时 syncing 布尔阻止反向写入。 |

## 完成记录

- 日期：2026-10-02
- commit：`ed63811`
- 设计文档处置：已部分删除声明的模型组合 / 角色 / 文件树与模型列表句；登记在 `docs/specs/design-deletions.md` 的「已部分删减的文档」，主题 / 快捷键 / 窗口段落保留。
