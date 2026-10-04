# S08-07 打包脚本（Windows）

> 状态: `todo`
> Phase: 08
> 依赖: S08-04, S08-09
> 阻塞: —
> 退役设计文档: —

## 目标

在 Windows 上产出安装包与更新包，签名并接入 `scripts/release/` 的清单与上传流程。

## 输入

- `docs/tasks/v2.0.0-gpui/08-release.md` D17
- `docs/release-workflow.md` §2–§4（清单格式、OSS 目录、发版步骤）
- `scripts/release/manifest.mjs`（需增加 `windows-x86_64` 平台）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| D17 | 打包 | 产出 `windows/x86_64/` 下的安装包与 `.sig`，上传后再合并进同一份清单 |

## 验收标准

- [ ] 同一版本的 macOS 与 Windows 制品出现在同一份 `stable.json`
- [ ] 新机器安装冒烟通过

## 证据

| 项 | 证据 |
|----|------|
| | |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| | | |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
