# S02-04 storage 移植与数据目录

> 状态: `doing`
> Phase: 02
> 依赖: S02-03
> 阻塞: —
> 退役设计文档: `docs/design/storage-design.md`

## 目标

把 v1 的 `storage.rs` + `storage/`（722 行）迁入 `buddy-engine`，去掉 `tauri::AppHandle`，改为显式传入数据根目录；默认根目录与 v1 相同。

## 输入

- 移植源：tag `v1-final` 的 `src-tauri/src/storage.rs`、`storage/{atomic_file,attachment_files,config_files}.rs`
- v1 `src-tauri/tauri.conf.json:5` `identifier = com.buddy.chat`
- `docs/evidence/s00-08/engine-integration.md` §5

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S02-04-1 | 去 Tauri | `storage.rs` 12 处 `AppHandle` 参数 → 数据根目录（`&Path` 或 `Storage` 结构体持有） |
| S02-04-2 | 默认目录 | macOS `~/Library/Application Support/com.buddy.chat`，Windows `%APPDATA%\com.buddy.chat`（与 Tauri `app_data_dir` 一致） |
| S02-04-3 | 原子写 / 附件 / 分文件配置 | 原样移植（D19-D21） |
| S02-04-4 | 不做历史迁移 | 用户决策（2026-09-26）：开发阶段，不做旧数据回归（原 D22 取消） |

## 验收标准

- [ ] storage 随迁单测全部通过，数量与 v1 一致
- [ ] 往返测试：临时目录内 写配置/消息 → 读回 → 相等（T04）
- [ ] 默认目录解析结果在本机打印为 `~/Library/Application Support/com.buddy.chat`
- [ ] 分层检查 14 项通过；v1 `cargo check` 通过
- [ ] `storage-design.md` 已处置并登记；数据目录的兼容性理由已迁到代码注释

## 证据

| 项 | 证据 |
|----|------|
| 单测 / 往返 | |
| 默认目录 | |
| 设计文档处置 | |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 历史数据回归 | 不做 | 用户：「当前还在开发阶段，不用考虑历史数据迁移」 |
| 默认目录 | 仍与 v1 相同 | 零成本；且两版并存期间可共用配置 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
