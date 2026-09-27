# S03-05 平台字体栈与 TextRenderingMode

> 状态: `blocked`
> Phase: 03
> 依赖: S03-04
> 阻塞: 等待用户目检（`handoff.md` §6.5 第 3 项：主题预览窗口中的字体观感是否与 v1 一致）
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

- [ ] 本机解析出的实际 UI 字体与 v1 一致（Fira Code，中文回退 PingFang）—— 程序化已确认，**待用户目检**
- [x] `text_rendering_mode()` 读回为 `Grayscale`

## 证据

| 项 | 证据 |
|----|------|
| 本机字体 | 自检输出：`installed(Fira Code)=true installed(PingFang SC)=true installed(JetBrains Mono)=false installed(Inter)=false`；`ui=Fira Code fallbacks=[".SystemUIFont", "PingFang SC", "Hiragino Sans GB"] mono=Fira Code mono_fallbacks=["Menlo"]` |
| 真正解析 | `resolve_font(ui) != resolve_font(".ZedMono")`（GPUI 解析失败时的全局回退首项）→ PASS |
| 渲染模式 | `PASS S03-05 text_rendering_mode 读回 Grayscale` |
| 单测 | `picks_first_installed_like_webkit` / `falls_through_to_system_ui_when_fira_is_missing` / `mono_uses_default_when_nothing_installed`；首版测试即抓到 `-apple-system` 与 `BlinkMacSystemFont` 重复映射的缺陷 |
| GPUI 字体名陷阱 | `all_font_names()` 无条件并入 GPUI 内置回退栈族名 → 首版回退列表出现未安装的 `Segoe UI`；非 Windows 平台剔除后消失 |
| 提交 | `8cadce6` |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 运行时选字体 | 取栈中第一个已安装者（WebKit 规则） | GPUI 首选字体未安装时退到自有全局栈（`.ZedMono` / Helvetica），不沿 CSS 链 → 会与 v1 不一致 |
| 字体不随包分发 | 与 v1 一致 | v1 无 `@font-face`；是否打包 Fira Code 属产品决策 → 用户决策清单 |
| TextRenderingMode | `Grayscale` | macOS 现状即灰度；为 Windows（R4）预留同口径 |
| 字距（letter-spacing） | **GPUI 本 rev 不支持**，`.t-title` 的 -0.01em 无法应用 | `styled.rs` 无对应方法；20px 下约 -0.2px/字，目检时关注标题是否偏松 |

## 完成记录

- 日期：
- commit：
- 设计文档处置：
