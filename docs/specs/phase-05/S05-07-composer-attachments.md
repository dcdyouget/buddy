# S05-07 Composer 附件（粘贴 / 拖拽 / 选择 / 草稿）

> 状态: `doing`
> Phase: 05
> 依赖: S05-06
> 阻塞: —
> 退役设计文档: —

## 目标

图片附件：粘贴、拖拽、文件选择；缩略图与删除；草稿（文字 + 图片）在页面切换后保留。

## 输入

- v1 `InputDock.tsx`、`AttachmentImage.tsx`、chatStore `draftInput` / `draftImages`、engine `save_chat_image` / `delete_chat_image`

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S05-07-1 | 三种来源 | 粘贴 / 拖拽 / 选择 |
| S05-07-2 | 模型不支持图片 | 发送时报「当前模型不支持图片，请移除图片或切换模型」（v1 原文） |

## 验收标准

- [x] 草稿保持测试
- [x] 附件交互 —— 用户 2026-10-01 授权 agent 本地读取真实渲染结果验收（handoff §6.5 第 22 项）

## 证据

| 项 | 证据 |
|----|------|
| 单测 | `cargo test -p buddy-ui chat::attachments`：MIME 白名单、扩展名推断、5 MB 边界、Base64 data URL；MIME、数量 / 大小限制与发送语义的拦截由 `scripts/chat/verify_phase05.py` 统一执行，结果见下方拦截记录。 |
| T33 | `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example chat_preview -- --selftest-attachments`。写入真实剪贴板、聚焦 TextArea、强制出帧，由 `window.dispatch_event(KeyDown(cmd-v))` 触发 Composer 的 `capture_action::<Paste>`；覆盖 4 张上限、第 5 张拒绝、视觉模型校验、纯图片发送、用户消息携图与发送后清空草稿。图片 Paste action 被消费，文本 Paste 继续传播给 TextArea。 |
| T34 | `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example app_preview -- --selftest-attachments` → `T34: 配置落盘 true；Cmd-V true；保存路径 true；切页保留 true；点击删除 true；文件清理 true；拖放入口 true；拖放保存 true；拖放删除 true；拖放清理 true；空态纯图页 true；纯图 Cmd-V true；纯图片保存 true；内存用户消息带图 true；磁盘用户消息带图 true；草稿清空 true；真实下载字节与清理 true；历史图片失败 true → 真实点击重试解码 true；实体销毁仍完成保存 true → 清理 true`，随后 `PASS S05-07 T34`。真实 `ChatEngine` 沙盒先持久化视觉模型配置，再分别经真实 Cmd-V 与 GPUI `FileDropEvent::Entered → Pending → Submit` 写入图片，覆盖异步 `save_chat_image`、设置页往返、缩略图关闭与 `delete_chat_image`；新建空态 Router 后发送空正文图片，分别核对内存会话与 engine 磁盘消息附件；历史附件临时改为缺失路径，先核对 GPUI 缓存返回 `Err`，修复文件后用真实鼠标点击重试，确认缓存解码为 `Ok` 且尺寸为 1×1。另调用生产 `download_generated_image`，核对系统下载目录的文件字节与原始 PNG 相等，并删除本次下载文件。实体销毁测试在同一前台任务内短暂暂停，先观察并核对 Tokio 实际写出的新文件，再让出前台使 detached 完成回调清理；避免保存未执行就把「文件不存在」算成清理成功。无入口兜底。 |
| 保存 / 删除 | `Composer` 选入 / 拖放 / 粘贴后经 `spawn_engine` 调用 `save_chat_image`；移除已写盘附件调用 `delete_chat_image`。保存期间发送按钮禁用。 |
| 原生文件选择 | `Composer` 的 ImagePlus 点击调用 GPUI `prompt_for_paths`，返回路径复用 `add_paths` 校验与保存；系统文件选择对话框内部：**该项无独立自动化证据**；原生 API 参数由代码复核，返回路径的校验 / 保存复用 T34 已验证的拖放链路。 |
| 本地渲染 | 用户 2026-10-01 授权 agent 自行验收。读取真实 GPUI 帧：普通 / 紧凑 Composer、视觉模型不支持提示、浅 / 深主题；真实 Transcript 的双列图片先于正文、自然比例与纯图片间距；历史图片失败占位及修复后真实点击重试的加载结果。强制出帧使用 `window.refresh(); window.draw(cx).clear(cx)`，临时 `render_to_image` 诊断代码 / feature / 图片不提交。 |
| 拦截验证 | `python3 scripts/chat/verify_phase05.py`：附件相关 10/10 被行为 FAIL 拦截（纯图发送、5 MB 边界、MIME 白名单、Cmd-V 入口、4 张上限、拖放入口、删除已保存图片、实体销毁清理、发送后清空草稿、历史图片重试）。每次先跑正常基线，再故意改坏一个实现；单测 rc=101 或 T33/T34 rc=1 且含对应 FAIL，编译错误与超时不算拦截；所有源码按原始字节还原。 |
| 最终回归 | `cargo test -q -p buddy-ui --lib`：119 passed；`NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example <名称> -- --selftest`：`chat_preview`、`pages_preview`、`app_preview`、`markdown_preview`、`streaming_preview` 均 rc=0 且包含对应 PASS。完整 chat 回归的 T33 按本次发送增量断言，避免前序测试记录造成误报；该修正后相关拦截再次通过。 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 草稿归属 | 保存在复用的 `Composer` 实体 | 空态页与对话页共用 Composer；切页不会丢失文字或图片草稿，与 v1 `chatStore` 共享草稿一致。 |
| 生命周期清理 | 保存完成任务 detach；弱 Composer 实体失效或用户提前移除时删除新保存的孤儿文件 | engine 写盘不会随 UI task 取消；完成与清理必须同样存活到写盘返回，路径先在 Tokio 上 canonicalize，满足 engine 的删除路径校验。 |
| 图片写盘 | 选入后立即异步调用 engine `save_chat_image`，发送仅携带路径附件 | 避免消息 JSON 嵌入 Base64；与 engine / v1 存储契约一致；异步任务经 `spawn_engine` 运行在 tokio。 |
| 输入入口 | 选择、GPUI 原生拖放、剪贴板图片三路归一到同一校验与保存逻辑 | spec 要求三种来源；v1 当前 `InputDock.tsx` 仅实现文件选择，粘贴 / 拖放属于补齐能力，已知行为差异记录在此。 |
| 缩略图 | Composer 使用 GPUI `Image` 的内存预览；历史用户消息使用路径优先、data URL 回退的共享 renderer | 路径附件在消息 JSON 中不含 Base64，渲染需按路径读取；无任何来源显示「图片已删除」；有路径但加载失败保留路径并提供重试，按 v1 区分永久缺失与临时失败。 |
| 紧凑窗口 | 视口高度不超过 180px 时把缩略图绝对定位到输入行左侧，并给文字与控制按钮预留宽度 | **偏离 v1**：v1 附件带位于输入区上方，紧凑态没有对应布局；此处在 560×60 中将图片放到输入行左侧；保持窗口不变尺寸（窗口调整归 Phase 07）同时让缩略图可删除、可发送。 |
| 紧凑提示 | 普通窗口显示「正在保存图片…」与不支持视觉模型提示；紧凑窗口通过输入区 tooltip 提示不支持视觉模型，保存提示不额外撑高 | **偏离 v1**：v1 的模型不支持提示持续显示在附件带内；此处紧凑态只显示 tooltip，保存提示不额外占行；窗口尺寸不能在 Composer 中调整，保留 60px 输入区的可编辑性。 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
