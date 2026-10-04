# S08-09 CI 流水线与版本一致性守卫

> 状态: `done`
> Phase: 08
> 依赖: S08-06
> 阻塞: —
> 退役设计文档: —

## 目标

一条命令 `npm run release` 在发布机上完成检查、版本写入、测试、构建、打包、签名、上传、公网回读、发布与打标签。

## 输入

- `docs/tasks/v2.0.0-gpui/08-release.md` D25–D29
- 用户 2026-10-04 要求：「运行一个脚本，输入更新内容和版本号，就自动打包、上传，固定地址拿到的就是最新下载地址」

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| D25 | 流水线 | 本机脚本 `scripts/release/release.sh`（9 步，见 `docs/release-workflow.md` §4），不使用云端 CI |
| D26 | 版本号 | `scripts/set-version.mjs` 改为写 `[workspace.package] version`；Info.plist、清单版本由同一值生成 |
| D27 | 上传 | ossutil；制品 immutable、清单 no-cache；全部从公网下载回来比对 sha256 |
| D28 | 一致性守卫 | 新版本必须高于线上 `stable.json`；标签不存在；Info.plist 版本 = 目标版本；工作区干净且在 main；试签验证私钥密码 |
| D29 | 缓存 | 复用本机 `target/`（release 增量构建约 45 秒） |

## 验收标准

- [x] 0.1.0 一次运行完成，固定地址返回 0.1.0 清单
- [x] 覆盖 `stable.json` 之前任一步失败，用户不受影响
- [x] 凭据不打印、不入库

## 证据

| 项 | 证据 |
|----|------|
| 失败停止 | 首次运行（旧私钥）在第 1 步试签失败：`错误：签名失败 …（检查私钥密码）`，无提交、无上传，`stable.json` 仍为 404 |
| 0.1.0 发布 | `npm run release -- 0.1.0 --notes … --yes` rc=0：提交 `78f10d2 chore(release): v0.1.0`，门禁「全部通过」，测试日志 18 组 `test result: ok`，上传并回读 sha256，覆盖 `channels/stable.json`，本地标签 `v0.1.0` |
| 线上读回 | `curl …/buddy/channels/stable.json`：HTTP 200、`Content-Type: application/json`、`Cache-Control: no-cache`，`version` 0.1.0、`notes` 与输入一致，`update` / `installer` / `source` 三个地址；DMG `HEAD` 200、`Content-Length: 14624731`、`Cache-Control: public,max-age=31536000,immutable` |
| 凭据 | 密码只经钥匙串 `buddy-updater-key` 传入子进程环境变量；脚本仅输出「读取自钥匙串」 |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 本机脚本而非云端 CI | 本机 | 签名私钥只留在发布机；macOS ARM 包本就需要 Mac 构建；用户要求一条命令 |
| 先提交版本号、发布后才打标签 | 是 | 中途失败重跑会跳过已提交的版本号；标签只指向真正发布的提交 |
| 标签不推送 | 是 | 用户 2026-09-27：整个 v2 完成后再推送 |
| 去掉对 S08-07 的依赖 | 依赖改为仅 S08-06 | macOS 流水线独立可用；Windows 制品由 S08-07 接入同一脚本（S08-07 依赖本项的脚本，而不是反过来） |

## 完成记录

- 日期：2026-10-04
- commit：`11c84bd`、`ea4c8f9`、`78f10d2`
- 设计文档处置：`docs/release-workflow.md` 按 v2 流程重写（非设计文档，为执行手册）
