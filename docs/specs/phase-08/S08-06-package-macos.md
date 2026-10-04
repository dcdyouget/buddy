# S08-06 打包脚本（macOS app + dmg）

> 状态: `done`
> Phase: 08
> 依赖: S08-03
> 阻塞: —
> 退役设计文档: —

## 目标

`scripts/release/bundle-macos.sh` 从 release 二进制组装 `Buddy.app`，产出应用内更新包（`.app.tar.gz`）与安装包（`.dmg`），取代 Tauri bundler。

## 输入

- `docs/tasks/v2.0.0-gpui/08-release.md` D15–D20
- v1 1.0.0 正式包的 Info.plist（bundle id `com.buddy.chat`、可执行名 `buddy`、最低 macOS 12.0）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| D15 | 组装 | `apps/buddy/bundle/macos/Info.plist` 模板（`@VERSION@`）+ `icon.icns`；Resources 附 LICENSE、GPL / Apache 全文、THIRD_PARTY_NOTICES.md；ad-hoc 签名并 `--strict` 自检 |
| D16 | DMG | `hdiutil` ULMO（LZMA），含 `Buddy.app` 与「应用程序」快捷方式；不做自定义背景（v1 也未配置背景） |
| D18 | 最低版本 | `MACOSX_DEPLOYMENT_TARGET=12.0`，Info.plist `LSMinimumSystemVersion` 12.0 |
| D19 | 架构 | 仅 Apple Silicon（沿用 v1 发布决定，不支持 Intel） |
| D20 | 体积 | `[profile.release] strip = true`；更新包 tar.xz、DMG ULMO（见下方体积构成） |
| — | 源码包 | `git archive HEAD` → `source/buddy-<版本>-src.tar.gz`（GPL） |

## 验收标准

- [x] 产物结构满足安装器校验（S08-03）：bundle id、版本、可执行文件、代码签名
- [x] DMG 校验通过，挂载后含 Buddy.app 与「应用程序」快捷方式，内部应用签名有效
- [x] 记录体积

## 证据

| 项 | 证据 |
|----|------|
| 0.1.0 产物 | `.release/0.1.0/macos/aarch64/`：`Buddy_0.1.0_aarch64.app.tar.gz` 12,208,540 B、`.dmg` 14,624,731 B、两个 `.sig`；`source/buddy-0.1.0-src.tar.gz` 2,017,488 B（含 `crates/markdown/VENDOR.md` 与 22 个 `crates/markdown/` 条目） |
| Bundle | `codesign -dv`：`Identifier=com.buddy.chat`、`Format=app bundle with Mach-O thin (arm64)`、`Signature=adhoc`；`otool -l` 的 `LC_BUILD_VERSION minos 12.0` |
| DMG | `hdiutil verify` → `checksum … is VALID`；只读挂载可见 `Applications -> /Applications` 与 `Buddy.app`，Resources 含 4 个许可文件与 icon.icns，`codesign --verify --deep --strict` 通过 |
| 体积 | 二进制 58,520,112 B（未 strip）→ 53,757,328 B（strip）。0.1.0 发布时为 gzip 更新包 12.2 MB / UDZO DMG 14.6 MB |
| 体积构成 | strip 后 53.5 MB 中 `__TEXT,__const` 30.15 MB、`__text` 17.41 MB。未 strip 符号聚合：tree-sitter 语法表 28.41 MB + 词法代码 1.90 MB。按语法静态库：C# 5.33、Swift 3.75、Kotlin 3.45、C++ 3.44、TypeScript+TSX 2.86、SQL 2.43、Ruby 2.11、PHP 2.06、Bash 1.37、Rust 1.12 MB，其余 16 个合计约 3.8 MB，总计 31.75 MB |
| 内置语法缩减 | 用户 2026-10-04 决定只内置 Python / Shell / SQL 语法，其余语言改用 `buddy_syntax::generic` 通用高亮：`cargo tree -p buddy-app -e normal` 只剩 tree-sitter-bash / python / sequel；release 二进制 53,757,328 → 27,252,176 B，更新包 tar.xz 6,198,068 B，DMG（ULMO）7,003,519 B |
| 压缩 | 同一 0.1.0 Buddy.app 实测：tar.gz 12,208,540 → tar.xz（bsdtar `xz:compression-level=9`）7,614,524 B；DMG UDZO 14,624,731 → ULFO 12,212,905 / UDBZ 12,467,174 / **ULMO 9,448,714 B**。`unpack_accepts_xz_and_gzip_archives` 单测确认安装器两种格式均可解；已发布 0.1.0 使用的 `tar -xzf` 实测可解 tar.xz（bsdtar 解包模式忽略 -z）并通过 `codesign --verify --strict` |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 不用 cargo-bundle / cargo-packager | 手写约 50 行脚本 | 产物结构必须与安装器校验逐项对应；脚本可读、无新工具依赖 |
| 模板放 `apps/buddy/bundle/macos/` | 不引用 `src-tauri/icons` | v1 工程未来删除时不影响 v2 打包 |
| tar 时 `COPYFILE_DISABLE=1` | 必须 | 否则 macOS tar 写入 `._*` 文件，解压后签名校验失败 |
| `Contents/MacOS` 只放 Mach-O | 必须 | 非 Mach-O 文件的签名存于扩展属性，`COPYFILE_DISABLE` 打包时丢弃，客户端严格校验会拒绝（单测改造中实际触发） |
| 压缩格式 | tar.xz + ULMO | 体积主体是可高度压缩的语法表；两者均为系统自带工具，不增加依赖；ULMO 需 macOS 10.15+，低于最低系统 12.0 |

## 完成记录

- 日期：2026-10-04
- commit：`11c84bd`
- 设计文档处置：无对应设计文档

## 备注

改用 xz / ULMO 并只内置三种语法后安装包 7.0 MB，满足 AGENTS.md 的 `< 10 MB` 目标；下一次发布生效（线上 0.1.0 仍为 14.6 MB）。
