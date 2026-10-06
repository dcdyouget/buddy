# 第三方声明

完整依赖与版本以 `Cargo.lock` 为准；各依赖的许可证全文随其源码包分发。

## Zed（GPL-3.0-or-later / Apache-2.0）

来源：https://github.com/zed-industries/zed ，rev `290cbcb9cb6a5dcbe0060431a126ad19e743f2f4`。

- GPL-3.0-or-later：`theme`、`ui`、`component`、`icons`、`menu`、`syntax_theme`、`ui_macros`，
  以及 vendored 的 `markdown`（`crates/markdown`）。全文见 `LICENSE-GPL-3.0-or-later`。
- Apache-2.0：`gpui` 及平台层、`collections`、`sum_tree`、`gpui_util` 等；
  `crates/markdown/util-shim` 内容取自 zed `util` / `gpui_util`。全文见 `LICENSE-APACHE-2.0`。
- 对 zed `markdown` 的修改以 patch 形式记录：`crates/markdown/patches/zed-markdown-290cbcb.patch`（说明见 `crates/markdown/VENDOR.md`）。

## Comet（MIT）

`crates/syntax` 移植自 Comet 的 `crates/syntax`，原许可证见 `crates/syntax/LICENSE-MIT-comet`。

## Lucide 图标（ISC）

`crates/ui/assets/icons/*.svg`（`streaming-star.svg` 除外，为本项目自绘）取自 Lucide，未修改图形。
Copyright (c) Lucide Icons and Contributors，许可证：https://lucide.dev/license

## global-hotkey（Apache-2.0 OR MIT）

`vendor/global-hotkey` 为 crates.io `global-hotkey 0.8.0` 的修改版：macOS Carbon 注册改为独占，
避免外部占键时静默成功。保留原许可证文件与版权声明，差异见该目录的 `VENDOR.md`。

## Microsoft Visual C++ Runtime（Microsoft 软件许可条款）

Windows 便携 ZIP 包含未修改的 `VCRUNTIME140.dll`，由发布脚本从已安装的 Visual Studio
`VC/Redist/MSVC/*/x64/Microsoft.VC143.CRT` 复制。该文件是 Microsoft Visual C++
Redistributable 的可分发代码，不适用 Buddy 的 GPL/MIT 许可证；其使用与再分发受
[Microsoft Visual Studio 许可条款](https://visualstudio.microsoft.com/license-terms/)及
[Visual Studio 可再发行组件说明](https://learn.microsoft.com/visualstudio/releases/2022/redistribution)约束。

## MPL-2.0 依赖

`cssparser`、`cssparser-macros`、`selectors`、`dtoa-short`、`option-ext` 等为 MPL-2.0（文件级 copyleft），
本项目未修改，仅链接使用。

## 其他

其余依赖均为 MIT / Apache-2.0 / BSD / ISC / Zlib / Unicode-3.0 等宽松许可。
