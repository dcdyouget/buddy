# S08-01 更新器选型与清单格式

> 状态: `done`
> Phase: 08
> 依赖: S07-11
> 阻塞: —
> 退役设计文档: —

## 目标

自研无 Tauri 依赖的更新 crate `crates/update`（MIT），定义固定清单地址、清单格式与版本选择规则。

## 输入

- `docs/tasks/v2.0.0-gpui/08-release.md` D01–D04
- 用户 2026-10-04 方案决定：OSS 原始域名、不经云服务器、产品介绍页从同一清单读取最新安装包地址；旧 v1 升级链路不再维护

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| D01 | 选型 | 自研 `buddy-update`：清单 / 下载 / 校验 / 安装共约 600 行，依赖均为 workspace 已有或极小 crate（`minisign-verify` 零依赖） |
| D02 | 清单格式 | `schema: 1`，每平台 `update`（应用内更新包）+ `installer`（DMG）各带 `url / size / sha256 / signature`；顶层 `source` 为 GPL 源码包 |
| D03 | 端点 | `https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/channels/stable.json`（`MANIFEST_URL`） |
| D04 | 版本比较 | `semver`；严格大于当前版本才提示；清单缺本平台按「已是最新」处理；不做渠道（只有 stable） |

## 验收标准

- [x] 清单格式由同一份规则生成（`scripts/release/manifest.mjs`）与解析（`crates/update/src/manifest.rs`）
- [x] 非法清单（JSON 无效、schema 不符、非 HTTPS、sha256 / size / 签名缺失）被拒绝
- [x] 线上固定地址返回的清单能被客户端解析并选出版本

## 证据

| 项 | 证据 |
|----|------|
| 单测 | `cargo test -p buddy-update`：`manifest_tests.rs` 5 项（新版本选中且带说明、同版本 / 旧版本为最新、0.10.0 > 0.9.0 数值比较、缺平台为最新、7 种非法清单拒绝） |
| 线上解析 | 见 S08-03 证据：探针以 0.0.9 读取线上 `stable.json`，选出 0.1.0 并打印更新说明 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 更新器 | 自研，而非 cargo-packager-updater 等库 | 需要的能力（静态清单 + 签名 + bundle 替换）很小；自研可直接加入 bundle id / 版本 / codesign 校验与流式回复中延迟重启，且不引入 Tauri 系依赖 |
| 清单托管 | OSS 静态文件，不经云服务器 | 服务器不在升级关键路径上，宕机不影响升级；用户决定不自定义域名 |
| 清单同时含安装包 | `installer` 字段 | 产品介绍页读取同一固定地址获得最新 DMG，下载地址无需另设跳转 |
| 不做灰度 rollout | 未实现 | 首发阶段用户量小，YAGNI；需要时在清单加字段，旧客户端忽略未知字段 |

## 完成记录

- 日期：2026-10-04
- commit：`11c84bd`、`ea4c8f9`
- 设计文档处置：无对应设计文档
