# buddy-engine

> **引擎层。MIT 许可。零 GPUI 依赖、零 GPL 链接。**

---

## 这个 crate 是什么

Buddy 的**全部业务逻辑**，不含任何界面代码：

| 模块 | 职责 | 迁入于 |
|------|------|--------|
| `models` | 数据模型（`AppConfig` / `Message` / `ModelInfo` / `MCP` 配置） | `S02-03` |
| `providers` | LLM provider 适配（OpenAI 兼容 / Anthropic） | `S02-01` |
| `streaming` | 流式事件（`StreamEvent` / `StreamOutcome`） | `S02-05` |
| `tools` | 工具调用（builtin / 图像生成 / 网络搜索 / 文件操作） | `S02-02` |
| `storage` | 持久化（JSON 分块 / 原子写 / 附件） | `S02-04` |

---

## 为什么必须保持纯净

### 许可证边界

界面层（`crates/ui`）依赖 zed 的 `theme` / `ui`，两者均为 **GPL-3.0-or-later**。

如果引擎层链接了它们，**整个 engine 也变成 GPL** ——
「以后把 engine 单独用于其他项目或闭源产品」的可能性就消失了。

因此引擎层与界面层之间有一条**硬边界**：

```text
buddy-engine   ← MIT，不依赖任何 GPL
     ↑
buddy-ui       ← GPL-3.0-or-later
     ↑
buddy-app      ← GPL-3.0-or-later（可执行程序整体受 GPL 约束）
```

**这条边界不是靠约定，而是靠 CI 断言守护** ——
`crates/engine` 的依赖树中不得出现 `gpui` / `theme` / `ui` / 任何 GPL crate。
见 `docs/specs/phase-01/S01-04-license-guard.md`。

### 已实测验证

**S00-08 实证**：本 crate 的依赖树里 0 处 GPUI / Tauri / zed crate。

```
$ cargo tree -p buddy-engine --depth 1
buddy-engine
├── async-trait      (proc-macro)
├── base64
├── chrono
├── dom_query
├── futures-util
├── log
├── parking_lot
├── reqwest
├── serde
├── serde_json
├── thiserror
└── tokio
```

全部为 MIT / Apache-2.0。**这不是设想，是已验证的边界。**

---

## 为什么没有 IPC 层

v1（Tauri）的架构是：

```text
React 前端 → invoke() → commands.rs（1868 行 IPC 胶水）→ provider
```

v2 的架构是：

```text
buddy-ui → 直接调用 buddy-engine 的类型与方法
```

**`commands.rs` 那 1868 行整体不需要了。** 跨 JS 边界才存在的 serde/IPC 层消失。

同理，流式事件不再走 Tauri event + JSON 序列化，而是直接 send 结构体
（`StreamEvent: Clone`）经 channel 到 UI。
见 `docs/evidence/s00-08/engine-integration.md`。

---

## 从 v1 移植的成本（已实测）

S00-08 把 v1 的引擎层搬入本 crate，**全部改动只有 4 处**：

| # | 文件 | 改动 |
|---|------|------|
| 1 | `streaming.rs` | `StreamEventEmitter`：`app: AppHandle` + `event_name` → `tx: UnboundedSender<StreamEvent>`（**唯一的 Tauri 耦合点，3 处**） |
| 2 | `providers/openai_compatible.rs` | edition 2021 → 2024 模式修正（1 处：`\|(_, &v)\|` → `\|&(_, &v)\|`） |
| 3 | `Cargo.toml` | 补 `parking_lot` |
| 4 | `tools` | 只取 `ToolDefinition` / `ToolSafety` 两个类型；完整模块见 `S02-02` |

引擎层规模约 **5255 行**。

---

## 设计约束

| 约束 | 原因 |
|------|------|
| **不依赖 `gpui` / `theme` / `ui`** | GPL 隔离（见上） |
| **不依赖任何 UI 框架** | 同上 |
| `#![forbid(unsafe_code)]` | 引擎层应可完全安全审计；`unsafe` 只出现在界面层的 objc2 窗口补丁中 |
| **不依赖 `tauri`** | v2 已移除 Tauri；引擎层不应残留 |

---

## 相关文档

| 文档 | 内容 |
|------|------|
| `docs/specs/phase-01/S01-01-workspace.md` | workspace 结构与依赖方向 |
| `docs/specs/phase-01/S01-03-licensing.md` | 许可证分层声明 |
| `docs/specs/phase-01/S01-04-license-guard.md` | 防 GPL 污染的 CI 断言 |
| `docs/evidence/s00-08/engine-integration.md` | 去 Tauri 化的集成规范与四个陷阱 |
| `docs/tasks/v2.0.0-gpui/research-log.md` §5 §17 | 各模块 Tauri 引用实测 / 引擎去 Tauri 化 |
