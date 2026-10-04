# S08-04 Windows 安装流程

> 状态: `todo`
> Phase: 08
> 依赖: S08-02
> 阻塞: —
> 退役设计文档: —

## 目标

Windows x86_64 下载已验证的安装包，静默安装并重启（对应 v1 `installMode: passive`）。

## 输入

- `docs/tasks/v2.0.0-gpui/08-release.md` D09
- `crates/update/src/lib.rs`：`install` / `relaunch_after_exit` 的非 macOS 分支当前返回「当前平台暂不支持自动更新」
- 用户 2026-10-03 决定：Windows 由用户在 Windows 环境执行（Phase 09）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| D09 | 安装 | 在 `crates/update` 增加 Windows 分支；清单 `platforms["windows-x86_64"]` 格式与 macOS 相同 |

## 验收标准

- [ ] 旧版本经设置页「立即更新」安装新版本并重启，版本读回正确
- [ ] 下载 / 校验失败不留下损坏安装

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
