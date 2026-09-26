# S03-05 平台字体栈与 TextRenderingMode

> 状态: `todo`
> Phase: 03
> 依赖: S03-04
> 阻塞: —
> 退役设计文档: —

## 目标

把 CSS 字体栈落为 GPUI 的 `Font { family, fallbacks }`，并显式设定 `TextRenderingMode`，记录理由。

## 输入

- 本机实测：`Fira Code` 已安装（11 个样式），`JetBrains Mono` / `Inter` 未安装 → v1 在本机实际用 Fira Code；v1 **未随包分发字体**（`global.css` 无 `@font-face`）
- GPUI：`FontFallbacks::from_fonts`、`App::set_text_rendering_mode`、`TextRenderingMode::{PlatformDefault, Subpixel, Grayscale}`

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| S03-05-1 | CSS 通用族映射 | `-apple-system` / `BlinkMacSystemFont` → `.SystemUIFont`；`sans-serif` / `monospace` 丢弃（GPUI 无通用族） |
| S03-05-2 | 不存在的字体 | 跳过并记日志，与 WebKit 行为一致 |
| S03-05-3 | TextRenderingMode | macOS 无次像素 AA；显式设 `Grayscale`（与 macOS 现状一致、为 Windows 预留同口径） |

## 验收标准

- [ ] 本机解析出的实际 UI 字体与 v1 一致（Fira Code，中文回退 PingFang）—— **需用户目检**
- [ ] `text_rendering_mode()` 读回为 `Grayscale`

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
