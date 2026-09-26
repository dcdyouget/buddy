# S00-08 引擎层最小闭环

> 状态: `done`
> Phase: 00
> 依赖: S00-01
> 阻塞: —
> 退役设计文档: —

## 目标

打通「真实 API 请求 → SSE 流式 → GPUI 界面渲染」的完整链路，验证 engine 层可脱离 Tauri 独立工作。

**产出物**：读 v1 真实配置 → 调用真实 provider → 流式渲染 markdown 到 GPUI 窗口。

## 输入

- `docs/tasks/v2.0.0-gpui/research-log.md` §5 —— 各模块 Tauri 引用实测（providers/tools/models 均为 0）
- `docs/design/sse-and-api.md` —— 流式与 API 约定
- `src-tauri/src/providers/`（2826 行，0 处 Tauri 引用）—— 直接复用
- `src-tauri/src/streaming.rs`（808 行，3 处 Tauri 引用）—— 需改造
- `~/Project/comet/docs/research/gpui.md` §7 —— `gpui_tokio` 桥接方式

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S00-08-1 | 抽出最小 engine crate | `buddy-engine`（MIT），models + providers + streaming |
| S00-08-2 | `streaming.rs` 改造 | **仅 3 处**：`AppHandle` → channel |
| S00-08-3 | tokio 桥接 | `gpui_tokio::init` + `Tokio::spawn` ✅ 可用 |
| S00-08-4 | 取消能力 | 创建了 `watch::channel`，**未实测触发**（→ `S02-05`） |
| S00-08-5 | usage 计数 | 保留（未改动） |
| S00-08-6 | 端到端串联 | ✅ provider → 流式 → S00-06 markdown |
| S00-08-7 | 反压观察 | 见证据（113 事件，约 20ms 轮询一次，无卡顿） |
| S00-08-8 | GPL 隔离验证 | ✅ **engine 依赖树 0 处 GPUI / Tauri / zed** |

**产物已固化**：`docs/evidence/s00-08/engine-integration.md`（改动清单 + 四个陷阱 + 集成骨架）。

## 验收标准

- [x] `cargo tree -p <engine>` 无 `tauri`
- [x] `cargo tree -p <engine>` 无 `gpui`（**风险 R7 前置验证**）
- [x] 真实 provider 请求成功并流式渲染到界面
- [ ] 取消生成可用，无残留任务 —— **未实测**（→ `S02-05`）
- [x] token 计数正确 —— 保留原有逻辑，未改动
- [x] 高频 token 流不造成界面卡顿（113 事件正常渲染）
- [x] 录屏证据完整 —— 改为**终端日志 + 用户确认**

## 证据

### 1. 端到端跑通 ✅

```
[config] model = MiniMax-M3  provider = MiniMax  base_url = https://api.minimaxi.com/v1  api_key = sk-cp-…
[done] full_text = 836 字符，thinking = 0 字符，tool_calls = 0，had_stream_error = false
[ui  ] 事件 = 113，文本增量 = 58（358 字符），思考增量 = 49（463 字符），终态携带 = 836 字符
```

链路：**v1 真实配置 → `create_provider` → 真实 API 请求 → SSE 流式 → 113 事件 → vendored markdown 渲染**

**用户实机确认**：「markdown 渲染正常」。

### 2. 从 v1 移植的**全部改动 = 4 处**

| # | 文件 | 改动 |
|---|------|------|
| 1 | `streaming.rs` | `StreamEventEmitter`：`app: AppHandle` + `event_name` → `tx: UnboundedSender<StreamEvent>`（3 处） |
| 2 | `providers/openai_compatible.rs` | edition 2024 模式修正（1 处，见 §4） |
| 3 | `Cargo.toml` | 补 `parking_lot` |
| 4 | `tools` | 只取 2 个类型（~40 行），完整模块归 `S02-02` |

**engine 规模 5255 行，改动 4 处。**

### 3. IPC 层消失 ✅

v1：UI → `invoke()` → `commands.rs`（**1868 行** IPC 胶水）→ provider
本 spike：UI → **直接调用** provider

`commands.rs` 整体不需要了。

### 4. ⚠️ 发现一：`Done.full_text` **含 ` thinking` 标签**，不是显示文本

```
文本增量 358 字符 + 思考增量 463 字符 = 821
终态事件（Done.full_text）携带       = 836
                                差 = 15
```

**` thinking` 7 字符 + `` 8 字符 = 15。** 差额精确对上。

**根因**（`openai_compatible.rs:674`）：

```rust
full_response.push_str(delta);            // ← 累积**原始** delta（未剥离 think）
emitter.text_delta(content_index, delta); // ← 这里才经 InlineThinkParser 拆分
```

| 来源 | 内容 | 用途 |
|------|------|------|
| `Done.full_text` | 原始文本（**含 think 标签**） | 持久化 |
| `TextDelta` 累加 | 显示文本（已剥离） | 渲染 |
| `ThinkingDelta` 累加 | 思考文本 | `ThinkSection` |

> **⚠️ 给 `S05-08` / `S05-09` 的警告**：**不要把 `Done.full_text` 当显示文本用**，
> 否则界面会出现字面的 ` thinking` 标签。

### 5. ⚠️ 发现二：内联 think 的字符数在 `StreamOutcome` 里**拿不到**

`StreamOutcome.thinking_text` 只统计**原生** reasoning 字段（DeepSeek `reasoning_content`），
**不含** `InlineThinkParser` 从 ` thinking` 标签提取的部分。

实测：`thinking_text = 0 字符`，但事件流里有 **49 次 `ThinkingDelta`、463 字符**。

> **给 `S05-09` 的提示**：思考块的字符数/折叠状态必须**在 UI 侧累加事件**。

### 6. ⚠️ 发现三：tokio 任务要求 `Send`，不能用 GPUI 侧类型

```
error: future cannot be sent between threads safely
  within async block, `Rc<std::cell::Cell<bool>>` is not `Send`
```

GPUI 是单线程模型（`Entity` / `Rc`），tokio 任务要求 `Send`。
**跨这个边界不能用 `Rc` / `Cell`。** 完成状态由事件流的 `Done`/`Error` 告知即可。

> 这是 `S02-06`（tokio / GPUI 桥接）的核心约束。

### 7. ⚠️ 发现四：spawn 到 tokio 的 future 需要 `'static`

```
error[E0597]: `config.providers` does not live long enough
```

需先把字段克隆出来（`compat` / `base_url` / `api_key`）再 move 进 future。

### 8. 风险 R7（GPL 传染）**实证可消除** ✅

```
gpui           ✅ 不在 engine 依赖树
tauri          ✅ 不在
zed_markdown   ✅ 不在
theme / ui     ✅ 不在
```

engine 的直接依赖只有：`reqwest` / `tokio` / `serde` / `serde_json` / `chrono` /
`dom_query` / `base64` / `parking_lot` / `futures-util` / `async-trait` / `thiserror` / `log`。

**分层不是设想，已验证可行。**

### 9. 顺带验证：v1 数据兼容零成本

直接读 `~/Library/Application Support/com.buddy.chat/config.json`，
拿到了 v1 的 `MiniMax-M3` 模型与 `MiniMax` provider，**无需任何迁移**。

一致性来自：同一路径 + 复用 `AppConfig` 结构 + `ProviderConfig` / `ModelInfo` 未改动。

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| engine 的 Tauri 去除方式 | **把 `StreamEventEmitter.app` 换成 channel** | 它是 streaming.rs 里唯一的耦合点（3 处）。且省掉了跨进程 JSON 序列化 |
| 事件传递机制 | **`tokio::sync::mpsc::unbounded_channel`** | `StreamEvent: Clone`（已 derive），可直接 send 结构体 |
| engine 的 `tools` 依赖 | **只取 2 个类型** | `providers/` 只用 `ToolDefinition` / `ToolSafety`；完整模块（~5000 行）非本 spec 所需 |
| tokio 桥接 | **`gpui_tokio::init` + `Tokio::spawn`** | 经 S00-01 调研推荐；实测可用 |
| 事件消费 | `try_recv` + 20ms 轮询（spike 用） | 简单可验证。**`S02-06` 应改为 `recv().await`** |
| 配置来源 | **直接读 v1 的真实配置** | 一次验证两件事：引擎可用 + 数据兼容零成本 |
| 测试提示词 | 要求返回「标题 + 列表 + 代码块 + 表格」 | 同时验证 S00-06 的 markdown 结构与流式 |
| 取消能力 | 创建 `watch::channel` 但**未触发** | 如实记录为未覆盖，交接 `S02-05` |
| 是否 patch GPUI | **否** | 全部改动都在 engine 侧（4 处），未触及 gpui |

## 完成记录

- 日期：2026-09-11
- commit：（spike 产物在 `spikes/`，已被 gitignore；**产物已固化到 `docs/evidence/s00-08/engine-integration.md`**）
- 设计文档处置：—（本 spec 不涉及设计文档退役）

## 遗留与交接

### 交给 `S02-05`（streaming 去 Tauri 化）

| 项 | 说明 |
|----|------|
| emitter 改造已验证 | 3 处改动，见产物 §1.1 |
| **取消生成（`cancel_rx`）未实测** | 本 spec 只创建了 watch channel |
| `Done.full_text` 含 think 标签 | 持久化与显示必须分开处理 |
| 反压 | 113 事件正常；更高频（如万 token 长回复）未测 |

### 交给 `S02-06`（tokio / GPUI 桥接）

| 项 | 说明 |
|----|------|
| **tokio 任务不能捕获 `Rc` / `Cell`** | 必须 `Send`；用 channel 传结果 |
| **future 需 `'static`** | 需克隆字段再 move |
| `try_recv` 轮询 → `recv().await` | 消除最长一个轮询周期的延迟 |
| `gpui_tokio::init` 只调用一次 | 本 spike 在 `application().run` 内调用 |

### 交给 `S02-02` / `S02-07`

完整 `tools` 模块（~5000 行）搬迁；工具调用链路本 spec **未实测**（传了空 `tools`）。

### 交给 `S02-09`

providers 的测试用例迁移。**本 spec 未跑任何 providers 的既有测试。**

### 交给 `S05-08` / `S05-09`

| 项 | 说明 |
|----|------|
| **不要用 `Done.full_text` 做显示** | 含 ` thinking` 标签 |
| **思考块字符数需 UI 侧累加** | `StreamOutcome.thinking_text` 不含内联 think |
| 多轮对话 / 历史消息 | 本 spec 只发了一条 user 消息 |

### 未覆盖清单

| 项 | 状态 |
|----|------|
| 取消生成 | 未实测 |
| 工具调用 | 未实测（空 tools） |
| 多轮对话 | 未实测 |
| providers 既有测试 | 未运行 |
| 存储层 | 未涉及（归 `S02-04`） |
| 高频长回复的反压 | 未测 |
| Windows 侧 | 归 `S09-*` |
