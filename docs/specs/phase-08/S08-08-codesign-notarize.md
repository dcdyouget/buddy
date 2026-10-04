# S08-08 签名与公证

> 状态: `dropped`
> Phase: 08
> 依赖: S08-06
> 阻塞: —
> 退役设计文档: —

## 废弃原因

用户 2026-10-04 决定不做 Developer ID 签名与公证，继续使用 ad-hoc 签名（影响见「备注」）。

## 目标

macOS 改用 Developer ID 签名并完成公证，让新用户从 DMG 打开时不被 Gatekeeper 拦截。

## 输入

- `docs/tasks/v2.0.0-gpui/08-release.md` D21–D24
- `scripts/release/bundle-macos.sh`：当前 `codesign --force --deep --sign -`（ad-hoc）

## 实现要点

| ID | 事项 | 说明 |
|----|------|------|
| D21 | 签名 | 需要用户的 Apple Developer 账号与 `Developer ID Application` 证书 |
| D22 | 公证 | `xcrun notarytool submit --wait` + `xcrun stapler staple`（DMG 与 .app） |
| D23 | 权限声明 | 确认全局热键 / 网络是否需要 entitlement |

## 验收标准

- [ ] `spctl --assess --type execute` 对 Buddy.app 通过
- [ ] 从公网下载的 DMG 首次打开无拦截

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

## 备注

当前 ad-hoc 签名的影响：应用内更新不受影响（下载文件不带隔离属性，安装器以 `codesign --verify --strict` 校验）；
新用户从 DMG 首次打开需右键「打开」或在「系统设置 → 隐私与安全性」中允许。
