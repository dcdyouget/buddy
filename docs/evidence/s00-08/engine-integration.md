# S00-08 产物：引擎层最小闭环的集成规范

> 完整证据见 `docs/specs/phase-00/S00-08-engine-loop.md`。
> 本文件记录**已被实测验证的集成方式**与**四个必须知道的陷阱**，供
> `S02-05`（streaming 去 Tauri 化）/ `S02-06`（tokio 桥接）/ `S05-09`（ThinkSection）取用。

---

## 0. 结论摘要

| 项 | 结果 |
|----|------|
| 引擎规模 | **5255 行**（models 1114 + providers 2826 + streaming 808 + 最小 tools ~40） |
| 对 v1 源码的改动 | **4 处**（见 §1） |
| engine 的 tauri 真实代码引用 | **0 处** |
| engine 依赖树里的 GPUI / Tauri / zed crate | **0 处**（风险 R7 已验证可消除） |
| 端到端 | ✅ 真实 API → SSE → 113 事件 → markdown 渲染，`had_stream_error = false` |
| IPC 层 | ✅ **完全消失**（UI 直接调用 provider） |

---

## 1. 从 v1 移植引擎层的**全部改动**（4 处）

### 1.1 `streaming.rs`：`StreamEventEmitter` 的 `AppHandle` → channel（3 处）

这是 streaming.rs 里**唯一**的 Tauri 耦合点。

```diff
- use tauri::Emitter;
+ use tokio::sync::mpsc::UnboundedSender;

  pub struct StreamEventEmitter {
-     app: tauri::AppHandle,
-     event_name: String,
+     tx: UnboundedSender<StreamEvent>,
      inline_think_parsers: Mutex<HashMap<usize, InlineThinkParser>>,
  }

- pub fn new(app: tauri::AppHandle) -> Self { … }
+ pub fn new(tx: UnboundedSender<StreamEvent>) -> Self { … }

  pub fn emit(&self, event: &StreamEvent) {
-     let _ = self.app.emit(&self.event_name, event);
+     let _ = self.tx.send(event.clone());   // StreamEvent: Clone 已有
  }
```

**顺带加一个便捷构造**：

```rust
pub fn channel() -> (Self, tokio::sync::mpsc::UnboundedReceiver<StreamEvent>) {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
    (Self::new(tx), rx)
}
```

**收益**：不再需要 JSON 序列化跨进程边界（原先是 emit 给 webview）。
1104 行的 `commands.rs` IPC 胶水整体消失。

### 1.2 `providers/openai_compatible.rs`：edition 2021 → 2024（1 处）

```diff
- .find(|(_, &v)| v == idx)
+ .find(|&(_, &v)| v == idx)
```

**原因**：edition 2024 禁止「在隐式借用模式里显式解引用」。
v1 是 edition 2021，所以原写法合法。**这是全量移植中唯一的代码级改动。**

> 移植时若遇到同类错误，模式是 `|(_, &v)|` → `|&(_, &v)|`。

### 1.3 `Cargo.toml`：补一个直接依赖

```diff
+ parking_lot = "0.12"
```

（v1 的 workspace 里有，移植时需要显式声明）

### 1.4 `tools` 模块：只取 2 个类型

`providers/` 只用到 `tools::{ToolDefinition, ToolSafety}`。
完整的 tools 模块（~5000 行）**本 spec 不需要**，用一个 ~40 行的最小子集即可。

`S02-02` 会把完整 tools 模块搬入。

---

## 2. 分层（S01-01 将正式确立）

```text
buddy-engine   ← MIT，零 GPUI / 零 Tauri / 零 GPL，可独立测试
    ↑
buddy-app      ← GPL，GPUI + zed theme/ui + vendored markdown
```

**实测验证**（`cargo tree -p buddy-engine`）：

```
gpui           ✅ 不在
tauri          ✅ 不在
zed_markdown   ✅ 不在
theme / ui     ✅ 不在
```

engine 的直接依赖只有：`reqwest` / `tokio` / `serde` / `serde_json` / `chrono` /
`dom_query` / `base64` / `parking_lot` / `futures-util` / `async-trait` / `thiserror` / `log`。

> **这是 GPL 隔离（风险 R7）的实证** —— 分层不是设想，已验证可行。

---

## 3. 四个陷阱

### 3.1 ⚠️ `Done.full_text` **含 ` thinking` 标签**，不是可直接显示的文本

**实测数据**：

```
[done] full_text = 836 字符，thinking = 0 字符
[ui  ] 文本增量 = 58（358 字符），思考增量 = 49（463 字符）
[ui  ] 文本增量 + 思考增量 = 821 字符
                                   差额 = 15
```

**` thinking` 是 7 字符、`` 是 8 字符 → 合计正好 15。差额由此而来。**

**根因**（`providers/openai_compatible.rs:674`）：

```rust
full_response.push_str(delta);           // ← 累积**原始** delta（未剥离 think）
emitter.text_delta(content_index, delta); // ← 这里才经 InlineThinkParser 拆分
```

所以：

| 来源 | 内容 |
|------|------|
| `StreamEvent::Done { full_text }` | **原始文本**（含 ` thinking` / `` 标签）→ **用于持久化** |
| `TextDelta` 累加 | 显示文本（已剥离 think） |
| `ThinkingDelta` 累加 | 思考文本 |

**关系**：`full_text` = `TextDelta` + `ThinkingDelta` + 标签开销

> **给 `S05-08` / `S05-09` 的警告**：**不要直接把 `Done.full_text` 当显示文本用**，
> 否则界面上会出现字面的 ` thinking` 标签。
> 显示必须用 `TextDelta` 累加；思考部分走 `ThinkingDelta`（`ThinkSection`）。

### 3.2 ⚠️ 内联 think 的字符数**在 `StreamOutcome` 里拿不到**

`StreamOutcome.thinking_text` 只统计**原生** reasoning 字段（如 DeepSeek 的 `reasoning_content`），
**不包含**由 `InlineThinkParser` 从 ` thinking` 标签里提取的部分。

实测：`thinking = 0 字符`，但事件流里有 **49 次 `ThinkingDelta`、463 字符**。

> **给 `S05-09` 的提示**：若要显示思考块的字符数/折叠状态，必须**在 UI 侧累加事件**，
> 不能依赖 `StreamOutcome`。

### 3.3 ⚠️ tokio 任务要求 `Send` —— 不能用 `Rc` / `Cell` 回传状态

```
error: future cannot be sent between threads safely
  within async block, `Rc<std::cell::Cell<bool>>` is not `Send`
```

GPUI 是单线程模型（`Entity` / `Rc`），而 tokio 任务要求 `Send`。
**跨这两个边界时不能用 GPUI 侧的类型。**

**正确做法**：
- 结果通过 **channel**（`StreamEvent`）传递
- 或让 tokio 任务返回 `Task<T>` 并在 GPUI 侧 `await`
- 完成状态由事件流的 `Done` / `Error` 告知，不需要额外的原子标志

> 这是 `S02-06`（tokio / GPUI 执行器桥接）的核心约束。

### 3.4 ⚠️ spawn 到 tokio 的 future 需要 `'static` 所有权

```rust
// ❌ 编译失败：`config.providers` does not live long enough
provider.stream_chat(…, provider_cfg.compat.as_ref(), …)

// ✅ 先把需要的字段克隆出来
let compat = provider_cfg.compat.clone();
… stream_chat(…, compat.as_ref(), …)
```

---

## 4. 推荐的集成骨架

```rust
// 1. tokio ↔ GPUI 桥接（在 GPUI 启动时初始化一次）
gpui_tokio::init(cx);

// 2. 创建 provider（providers/ 原样可用）
let ptype = ProviderType::from_str(&provider_cfg.provider_type);
let provider = create_provider(&ptype);

// 3. channel 取代 Tauri event
let (emitter, mut rx) = StreamEventEmitter::channel();
let (_cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);

// 4. ⚠️ 克隆出需要的字段（future 需要 'static）
let base_url = provider_cfg.base_url.clone();
let api_key = provider_cfg.api_key.clone();
let compat = provider_cfg.compat.clone();

let fut = async move {
    provider.stream_chat(&base_url, &api_key, &model, &rid, &messages,
                         &emitter, cancel_rx, compat.as_ref(), &[]).await
};
gpui_tokio::Tokio::spawn(cx, fut).detach();   // ⚠️ 不要捕获 Rc/Cell

// 5. 消费事件 → 喂入 markdown
cx.spawn(async move |this, cx| loop {
    loop {
        match rx.try_recv() {
            Ok(ev) => { /* TextDelta → append; ThinkingDelta → 思考块 */ }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => { finished = true; break; }
        }
    }
    …
    cx.background_executor().timer(Duration::from_millis(20)).await;
}).detach();
```

> **`S02-06` 应评估的更优方案**：用 `rx.recv().await` 代替 `try_recv` + 20ms 轮询，
> 消除最长一个轮询周期的延迟。

---

## 5. 复用 v1 真实配置（顺带验证数据兼容）

```rust
// v1 的 `tauri::AppHandle::app_data_dir()` 在 macOS 上即此路径
let dir = PathBuf::from(home).join("Library/Application Support/com.buddy.chat");
let config: AppConfig = serde_json::from_str(&fs::read_to_string(dir.join("config.json"))?)?;
```

实测：直接读到了 v1 的 `MiniMax-M3` 模型与 `MiniMax` provider，**无需任何迁移**。

> **`S02-04` 可据此确认**：只要新实现指向同一路径 + 复用 `AppConfig` 结构，数据兼容零成本。

---

## 6. 端到端实测结果

```
[config] model = MiniMax-M3  provider = MiniMax  base_url = https://api.minimaxi.com/v1
[done] full_text = 836 字符，thinking = 0 字符，tool_calls = 0，had_stream_error = false
[ui  ] 事件 = 113，文本增量 = 58（358 字符），思考增量 = 49（463 字符）
```

用户实机确认：**markdown 渲染正常**。

链路：**v1 真实配置 → `create_provider` → 真实 API 请求 → SSE 流式 → 113 事件 → markdown 渲染**。

---

## 7. 未覆盖 / 交接

| 项 | 交接 |
|----|------|
| 取消生成（`cancel_rx`） | **本 spec 未实测**（只创建了 watch channel，未触发取消）→ `S02-05` |
| 工具调用（`tool_calls`） | 本 spec 传空 `tools`，未实测 → `S02-02` / `S02-07` |
| 多轮对话 / 历史消息 | 只发了一条 user 消息 → `S05-*` |
| `try_recv` 轮询 → `recv().await` | `S02-06` |
| 完整 tools 模块（~5000 行） | `S02-02` |
| 存储层（`storage/`） | `S02-04` |
| providers 的测试用例 | `S02-09` |
