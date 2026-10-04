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
| D16 | DMG | `hdiutil` UDZO，含 `Buddy.app` 与「应用程序」快捷方式；不做自定义背景（v1 也未配置背景） |
| D18 | 最低版本 | `MACOSX_DEPLOYMENT_TARGET=12.0`，Info.plist `LSMinimumSystemVersion` 12.0 |
| D19 | 架构 | 仅 Apple Silicon（沿用 v1 发布决定，不支持 Intel） |
| D20 | 体积 | `[profile.release] strip = true` |
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
| 体积 | 二进制 58,520,112 B（未 strip）→ 53,757,328 B（strip）；DMG 14.6 MB（v1 Tauri 7.2 MB；GPUI 静态链接渲染与文字栈，超出 AGENTS.md「< 10 MB」目标，已登记） |

## 决策记录

| 决策 | 选择 | 理由 |
|------|------|------|
| 不用 cargo-bundle / cargo-packager | 手写约 50 行脚本 | 产物结构必须与安装器校验逐项对应；脚本可读、无新工具依赖 |
| 模板放 `apps/buddy/bundle/macos/` | 不引用 `src-tauri/icons` | v1 工程未来删除时不影响 v2 打包 |
| tar 时 `COPYFILE_DISABLE=1` | 必须 | 否则 macOS tar 写入 `._*` 文件，解压后签名校验失败 |

## 完成记录

- 日期：2026-10-04
- commit：`11c84bd`
- 设计文档处置：无对应设计文档

## 备注

安装包 14.6 MB 超过 AGENTS.md 的 `< 10 MB` 目标；是否放宽目标或继续压缩（LTO、codegen-units=1）留待用户决定。
