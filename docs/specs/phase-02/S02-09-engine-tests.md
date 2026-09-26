# S02-09 引擎测试迁移与契约测试

> 状态: `done`
> Phase: 02
> 依赖: S02-01, S02-02, S02-03, S02-04
> 阻塞: —
> 退役设计文档: —

## 目标

确认 v1 全部 Rust 引擎测试已在 `buddy-engine` 跑通（逐模块数量对照），并补上手动触发的真实 provider 契约测试。

## 输入

- v1 `cd src-tauri && cargo test`：166 passed / 8 ignored（S01-05 回退演练实测）
- `docs/tasks/v2.0.0-gpui/02-engine.md` T01 / T05 / T07

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S02-09-1 | 数量对照 | 按模块统计 v1 与 engine 的测试数；差额必须逐条解释（如属于 commands/window 等不迁模块） |
| S02-09-2 | 真实契约测试 | 两个 provider 各一次真实请求，`#[ignore]`，读默认数据目录的配置（T05） |
| S02-09-3 | CI | 纪律检查之外，CI 增加 `cargo test -p buddy-engine` |

## 验收标准

- [x] 模块级测试数对照表，差额全部有解释
- [x] `cargo test -p buddy-engine` 全绿
- [x] 真实契约测试手动运行一次通过（记录模型名与事件数，不记录 key）—— **仅 openai_compatible**；anthropic 用户配置中无此类 provider，未真实验证（见证据）

## 证据

| 项 | 证据 |
|----|------|
| 对照表 | 见下表（`cargo test -- --list` 按模块统计） |
| 全量 | `cargo test -p buddy-engine`：**175 passed / 0 failed / 10 ignored**（lib 165 含 8 ignored；集成 mock_sse 2、stream_cancel 4、storage_roundtrip 4、chat_flow 8、real_provider 2 ignored） |
| 真实契约 | `cargo test -p buddy-engine --test real_provider -- --ignored --nocapture`：`[openai_compatible] model=MiniMax-M3 provider=MiniMax events=25 text_deltas=1 thinking_deltas=18 full_text_chars=249 had_stream_error=false`；`[anthropic] 配置中没有可用的该类 provider，跳过` → **Anthropic 协议仅有 mock 验证**（S02-01 / S02-05），无真实请求证据 |
| 联网 ignored 用例 | engine `--ignored`：3 passed / 5 failed；v1 `src-tauri` 同批：4 passed / 4 failed。失败集合相同（bing 实体搜索、bing query 相关性、so360、websearch 结构化输出），代码逐字节一致；差异项 `live_websearch_handles_current_stock_query` 在 engine 连跑 3 次为 失败/通过/通过（实时结果不稳定）。bing 失败原因：`"Bing 中国返回了不受信任或 query 不一致的搜索重定向"` → **v1 既有问题，非迁移回归** |
| CI | `.github/workflows/discipline.yml` 新增「引擎测试」步骤 `cargo test -p buddy-engine`（ignored 用例不运行） |
| 提交 | `755fc44` |

| 模块 | v1 `src-tauri`（含 ignored） | engine lib（含 ignored） | 差额解释 |
|------|---------|---------|---------|
| `commands` → `chat` | 19 | 18 | `frontend_diagnostic_accepts_only_known_stages`（窗口诊断）→ Phase 07 |
| `models` | 21 | 21 | — |
| `providers` | 26 | 26 | — |
| `storage` | 11 | 11 | — |
| `streaming` | 8 | 8 | — |
| `tools` | 81（8 ignored） | 81（8 ignored） | — |
| `window` | 8 | 0 | 窗口定位属应用外壳 → Phase 07 |
| **合计** | **174** | **165** | 9 = 窗口相关，全部归 Phase 07 |
| engine 新增集成测试 | — | 20（2 ignored） | mock_sse / stream_cancel / storage_roundtrip / chat_flow / real_provider |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 真实契约测试不经 `ChatEngine` | 直接调 provider | `ChatEngine` 会把消息写进用户真实数据目录（与 v1 共用）；契约测试只需验证协议 |
| 联网用例不进 CI | 保持 `#[ignore]` | v1 即如此；实时搜索结果不稳定，且 bing 在当前网络下本就失败 |

## 完成记录

- 日期：2026-09-27
- commit：`755fc44`
- 设计文档处置：—

## 备注

⚠️ 交给用户确认：v1 的联网搜索在当前网络下 bing / so360 测试失败（「不受信任或 query 不一致的搜索重定向」），可能意味着 v1 的 websearch 工具在实际使用中也受影响。不属于迁移范围。
