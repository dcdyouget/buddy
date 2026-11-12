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

## LaTeX-Rust 数学排版（MIT OR Apache-2.0）

`latex-rust 2.1.1` 将公式排版为字体轮廓 SVG，来源：https://github.com/jscarr64/LaTeX-Rust 。
Rust 源码按 Apache-2.0 使用，全文见 `LICENSE-APACHE-2.0`。内嵌 STIX Two Math 字体按 SIL OFL 1.1 使用，
颜色表按 LPPL 1.3c 或更新版本使用。以下保留上游 NOTICE、颜色表修改说明与字体许可，随应用一并分发。

### LaTeX-Rust NOTICE

```text
LaTeX-Rust
Copyright 2026 Jeffrey S Carr

Licensed under MIT OR Apache-2.0.

This product embeds STIX Two Math (version 2.13), licensed under the
SIL Open Font License 1.1. See fonts/stix-two-math/OFL.txt.

STIX Fonts is a trademark of The Institute of Electrical and
Electronics Engineers, Inc.

This product includes a modified copy of the dvips color names
(dvipsnam.def), taken from the `%<*dvipsnames>` section of drivers.dtx
in the LaTeX graphics bundle.

Copyright 1994 David Carlisle and Sebastian Rahtz
Copyright 1995-1999 David Carlisle
Copyright 2000-2026 The LaTeX Project

That material is licensed under the LaTeX Project Public License,
version 1.3c or any later version. The latest version of the license
is at https://www.latex-project.org/lppl.txt and version 1.3c or later
is part of all distributions of LaTeX version 2008 or later.

data/dvipsnames.tsv is the modified file. It is clearly marked as
changed in its own header: the definitions were rewritten as a
tab-separated table and the rows were re-sorted alphabetically by
name. Upstream dvipsnam.def keeps the dvips color-wheel order. The
CMYK numbers, including TealBlue (0.86, 0, 0.34, 0.02), match
drivers.dtx. The file is named dvipsnames.tsv so it is not a drop-in
replacement for dvipsnam.def under LaTeX. The original authors and
The LaTeX Project do not maintain or support this file.

A complete unmodified copy of the graphics bundle, including
drivers.dtx, is available from
https://github.com/latex3/latex2e/tree/develop/required/graphics
(CTAN: macros/latex/required/graphics).

The MIT and Apache-2.0 terms apply to the Rust source. They do not
relicense data/dvipsnames.tsv or the STIX font. Anyone who distributes
the color table must keep the header in data/dvipsnames.tsv and this
notice, so the file stays identified as a modified work and a recipient
can still obtain the original graphics bundle.
```

### STIX Two Math — SIL Open Font License 1.1

```text
Copyright 2001-2021 The STIX Fonts Project Authors (https://github.com/stipub/stixfonts)

STIX Fonts™ is a trademark of The Institute of Electrical and
Electronics Engineers, Inc.

Portions copyright © 1998–2003 by MicroPress, Inc.
(www.micropress-inc.com), with Reserved Font Name TM Math.
To obtain additional mathematical fonts, please contact
    MicroPress, Inc.,
    68-30 Harrow Street,
    Forest Hills, NY 11375,
    USA
    Phone: (718) 575-1816.

Portions copyright © 1990 by Elsevier, Inc.

Portions copyright © 2010, 2012, 2014 Adobe Systems Incorporated.

This Font Software is licensed under the SIL Open Font License, Version 1.1.
This license is copied below, and is also available with a FAQ at:
http://scripts.sil.org/OFL

-----------------------------------------------------------
SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007
-----------------------------------------------------------

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The
fonts, including any derivative works, can be bundled, embedded,
redistributed and/or sold with any software provided that any reserved
names are not used by derivative works. The fonts and derivatives,
however, cannot be released under any other type of license. The
requirement for fonts to remain under this license does not apply
to any document created using the fonts or their derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may
include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical
writer or other person who contributed to the Font Software.

PERMISSION & CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining
a copy of the Font Software, to use, study, copy, merge, embed, modify,
redistribute, and sell modified and unmodified copies of the Font
Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components,
in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy
contains the above copyright notice and this license. These can be
included either as stand-alone text files, human-readable headers or
in the appropriate machine-readable metadata fields within text or
binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font
Name(s) unless explicit written permission is granted by the corresponding
Copyright Holder. This restriction only applies to the primary font name as
presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font
Software shall not be used to promote, endorse or advertise any
Modified Version, except to acknowledge the contribution(s) of the
Copyright Holder(s) and the Author(s) or with their explicit written
permission.

5) The Font Software, modified or unmodified, in part or in whole,
must be distributed entirely under this license, and must not be
distributed under any other license. The requirement for fonts to
remain under this license does not apply to any document created
using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are
not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE
COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.
```

## Microsoft Visual C++ Runtime（Microsoft 软件许可条款）

Windows 安装包（`_setup.exe`）与便携 ZIP 包含未修改的 `VCRUNTIME140.dll`，由打包脚本从已安装的 Visual Studio
`VC/Redist/MSVC/*/x64/Microsoft.VC143.CRT` 复制。该文件是 Microsoft Visual C++
Redistributable 的可分发代码，不适用 Buddy 的 GPL/MIT 许可证；其使用与再分发受
[Microsoft Visual Studio 许可条款](https://visualstudio.microsoft.com/license-terms/)及
[Visual Studio 可再发行组件说明](https://learn.microsoft.com/visualstudio/releases/2022/redistribution)约束。

## MPL-2.0 依赖

`cssparser`、`cssparser-macros`、`selectors`、`dtoa-short`、`option-ext` 等为 MPL-2.0（文件级 copyleft），
本项目未修改，仅链接使用。

## 其他

其余依赖均为 MIT / Apache-2.0 / BSD / ISC / Zlib / Unicode-3.0 等宽松许可。
