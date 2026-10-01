# S05-12 图片生成卡片

> 状态: `done`
> Phase: 05
> 依赖: S05-10
> 阻塞: —
> 退役设计文档: —

## 目标

生成中占位、结果图、保存 / 复制提示词，与 v1 一致。

## 输入

- v1 `GenerateImageSection.tsx`、engine `download_generated_image`

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S05-12-1 | 保存 | 调用 engine |

## 验收标准

- [x] 观感 —— 用户 2026-10-01 授权 agent 本地读取真实渲染结果验收（handoff §6.5 第 21 项）

## 证据

| 项 | 证据 |
|----|------|
| 解析与状态文案 | `crates/ui/src/chat/image_gen_state.rs` 的单测 `parses_prompt_and_result_like_v1`、`labels_match_v1`覆盖合法 / 非法 JSON、提示词清理、结果元数据、生成中 / 完成 / 失败 / 中断文案。 |
| 图片展示 | `Transcript` 从对应 tool 消息恢复完整 `ImageAttachment`，生成卡片优先使用 engine 已落盘的 `path`，HTTP URL 复用 `markdown::gfm::image_source`，兼容 data URL 由现有附件解码链路转为 `ImageSource::Image`；预览 fixture 读取仓库 PNG 后转 data URL，不联网。 |
| T32 真实交互 | `NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example chat_preview -- --selftest-image`：真实 `window.dispatch_event` 鼠标事件验证生成中 / 完成、异步下载失败后重试成功、展开 / 收起、剪贴板提示词及 1.6s 反馈恢复。独立自测不注册分页 loader，场景重置包括完整历史分页状态；点击区域局限于读图确认的可见按钮，避免误点标题改变折叠状态。 |
| 图片加载重试 | T32 将结果切换为不存在的本地 Resource，必须先观察 GPUI cache 的 `Failed`，随后写入真实 PNG、鼠标点「重试」，确认 cache 返回实际解码的 `Loaded { width, height }`；同时核对失败占位 / 加载后行高变化、重测计数与 Tail 贴底。失败与加载中占位使用确定宽高；cache 状态变化后 defer 重测，避免在 ListState 布局锁内修改列表。普通构建输出：失败 / 加载后行高 `460 → 576`，异步重测与 Tail 保持均为 `true`。 |
| 真实下载边界 | `app_preview -- --selftest-attachments` 的 T34 调用生产 `ChatEngine::download_generated_image`，读取系统下载目录中的结果，与原始 PNG 逐字节相等，并删除本次下载文件；卡片交互的可控失败 / 成功由 T32 覆盖。 |
| 本地视觉验收 | 读取真实 GPUI 帧：生成中、完成折叠、展开元数据、图片加载失败占位及重试、浅 / 深主题；750×640 的真实 Transcript 同时展示 800×1200 竖图与 1200×800 横图，均按自然比例缩放并排；560×760 的卡片验证下载与复制反馈。出帧为 `window.refresh(); window.draw(cx).clear(cx)`；临时 `render_to_image` 代码、feature、锁文件变化与图片不提交。 |
| 拦截验证 | `python3 scripts/chat/verify_phase05.py`：生图相关 7/7 被 T32 的行为 FAIL 拦截（专用卡片分派、展开、复制提示词、失败占位尺寸、异步行高重测、清 cache 后重试、下载成功反馈）。均先通过正常基线，变异后 rc=1 且输出 `FAIL S05-12 T32`，编译错误 / 超时不算；源码按原始字节还原。最后对齐 v1 保存转圈、错误色与视口尺寸限制后，7 项复跑仍全部拦截。 |
| 最终回归 | `cargo test -q -p buddy-ui --lib`：119 passed；`NO_PROXY=127.0.0.1,localhost cargo run -q -p buddy-app --example <名称> -- --selftest`：`chat_preview`、`pages_preview`、`app_preview`、`markdown_preview`、`streaming_preview` 均 rc=0 且包含对应 PASS。普通构建验证 T32 全部布尔断言，未保留 test-support 或临时渲染诊断。 |
| 提交门禁 | 按项目代理配置运行 `scripts/gate.sh`，退出码 0；v1 源码、根 Cargo.toml / Cargo.lock 无改动。 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 图片来源 | engine 持久化后的 `ImageAttachment.path` 优先，HTTP 图片复用 `markdown::gfm::image_source` | 生图 Base64 已由 engine 在 ToolResult 前落盘；UI 不重复解码或新增依赖，与现有 HTTP 客户端一致。 |
| 卡片分派 | `generate_image` 独立卡片；图片网格常驻，详情由独立折叠状态管理，初始为收起 | 对齐 v1 `GenerateImageSection`：结果图直接可见，提示词 / 模型 / 优化提示词在展开详情中查看。 |
| 保存动作 | Transcript 提供可注入的异步 `DownloadFn`；PageRouter 与预览分别注入真实 engine 下载和沙盒 engine 下载 | 保存状态由卡片统一显示保存中 / 已下载 / 重试及错误；生产与预览共用 `ChatEngine::download_generated_image`，自测另注入可控失败 / 成功任务验证交互。 |
| 图片尺寸 | 本地 / HTTP Resource 使用现有 GPUI cache 异步解码后的实际尺寸计算缩放 | 避免每帧同步读大图；限制沿用 v1 的 480px 最大宽度与视口高度公式，支持横图、竖图、多图；失败 / 加载中的 Resource 没有固有尺寸，必须给占位确定尺寸。 |
| 背景与下载按钮 | 图片区用 `bg_sunken` 实色；下载按钮用状态色的半透明实色 | **偏离 v1**：未复刻图片区的径向渐变和下载按钮背景模糊；沿用现有 Theme 令牌与 GPUI 元素，不增加自定义着色器，图像及按钮轮廓、尺寸、交互保持。 |

## 完成记录

- 日期：2026-10-01（用户授权 agent 本地读取实际渲染结果完成验收）
- commit：`f6542b6`
- 设计文档处置：本 spec 未指定独立退役文档；聊天组件映射的部分退役随实现提交，见 `docs/specs/design-deletions.md`「已部分删减的文档」。
