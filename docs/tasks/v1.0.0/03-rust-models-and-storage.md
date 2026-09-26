# Task 03: Rust Models & Storage

## 目标

实现 Rust 数据结构定义和本地 JSON 文件存储系统。

## 相关设计文档

- `docs/design/rust-data-models.md` — 所有 struct 定义
- `docs/design/storage-design.md` — 文件布局、chunk 机制

## 验收标准

- [ ] `models.rs` 包含所有 struct：`AppConfig`, `ProviderConfig`, `ModelInfo`, `Message`, `Manifest`, `ChunkMeta`, `ChatChunk`
- [ ] 所有 struct 实现 `Serialize + Deserialize + Clone + Debug`
- [ ] `AppConfig::default()` 返回合理的默认值
- [ ] `storage::get_config()` 读取 `config.json`，不存在时返回默认
- [ ] `storage::save_config()` 写入 `config.json`
- [ ] `storage::append_message()` 正确管理 chunk：满 100 条自动创建新 chunk
- [ ] `storage::load_messages(offset, limit)` 正确分页跨 chunk 查询
- [ ] `manifest.json` 保持准确
- [ ] 数据存储路径使用 `app.path().app_data_dir()`

## 开发工作

| ID | Task | File | Details |
|----|------|------|---------|
| D07 | Define data models | `src-tauri/src/models.rs` | All structs per design doc |
| D08 | Config read/write | `src-tauri/src/storage.rs` | `get_config()`, `save_config()` |
| D09 | Chat chunk storage | `src-tauri/src/storage.rs` | `append_message()`, `load_messages()` |

## 测试工作

| ID | Task | Details |
|----|------|---------|
| T01 | Storage unit tests | Test chunk rotation at exactly 100 messages, verify manifest consistency, test paginated read across chunk boundaries |

写 `#[cfg(test)] mod tests` 在 `storage.rs` 底部，覆盖：

1. 空目录首次写入
2. 追加消息 < 100 条 → 不创建新 chunk
3. 追加消息 = 100 条 → 当前 chunk 关闭，新 chunk 创建
4. `load_messages(0, 50)` 正确返回前 50 条
5. `load_messages(150, 30)` 正确跨 chunk 返回
6. config 读/写 round-trip
7. config 文件损坏时返回默认值
