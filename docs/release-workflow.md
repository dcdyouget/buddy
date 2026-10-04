# Buddy 发布与自动更新手册（v2 GPUI）

> 更新时间：2026-10-04
> 适用：macOS Apple Silicon（`darwin-aarch64`）。Windows 由 Phase 09 补齐（S08-04 / S08-07）。
> v1（Tauri）的发布与 Updater 流程已废弃：v1 签名私钥密码遗失，v2 换用新密钥对，v1 客户端不再接收更新。

## 1. 两个固定地址

| 用途 | 地址 | 内容 |
|---|---|---|
| 版本信息（客户端检查更新、产品介绍页读取最新下载地址） | `https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/channels/stable.json` | 最新版本清单 |
| 制品 | `https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/releases/<版本>/...` | 每个版本一个目录，上传后不再修改 |

清单地址写死在客户端（`crates/update/src/lib.rs` 的 `MANIFEST_URL`）。**已发布客户端无法改地址**：
bucket `buddy-release`（cn-beijing）不能删除、改名或换地域；将来换地址只能在新版本客户端中进行，
并让旧地址继续发布，直到旧版本用户都已升级。

产品介绍页的「下载最新版」读取清单中的 `platforms["darwin-aarch64"].installer.url`。

## 2. 清单格式（`schema: 1`）

```json
{
  "schema": 1,
  "version": "0.1.0",
  "pub_date": "2026-10-04T03:20:00Z",
  "notes": "中文更新说明",
  "platforms": {
    "darwin-aarch64": {
      "update":    { "url": ".../Buddy_0.1.0_aarch64.app.tar.xz", "size": 0, "sha256": "…", "signature": "…" },
      "installer": { "url": ".../Buddy_0.1.0_aarch64.dmg",        "size": 0, "sha256": "…", "signature": "…" }
    }
  },
  "source": { "url": ".../source/buddy-0.1.0-src.tar.gz", "size": 0, "sha256": "…" }
}
```

- 生成：`scripts/release/manifest.mjs`；解析：`crates/update/src/manifest.rs`。不兼容的格式变更必须提升 `schema`。
- `signature` 是 minisign 签名文件全文的 base64（`tauri signer sign` 产出的 `.sig` 内容）。
- 客户端只在 `version` 严格高于当前版本且包含本平台时提示更新。

## 3. OSS 目录

```
buddy/
  channels/stable.json                       ← 发布开关（no-cache），唯一会被覆盖的对象
  releases/<版本>/
    manifest.json                            ← 本版本清单存档
    macos/aarch64/Buddy_<版本>_aarch64.app.tar.xz(.sig)   ← 应用内更新包（0.1.0 为 .tar.gz）
    macos/aarch64/Buddy_<版本>_aarch64.dmg(.sig)          ← 手动安装包
    source/buddy-<版本>-src.tar.gz           ← GPL-3.0 源码
```

制品使用 `Cache-Control: public,max-age=31536000,immutable`；清单使用 `no-cache`。

## 4. 发版

```bash
npm run release                                         # 交互输入版本号与更新说明
npm run release -- 0.2.0 --notes "修复 xxx；新增 yyy"
npm run release -- 0.2.0 --notes-file notes.txt --yes
```

脚本 `scripts/release/release.sh` 依次执行：

| 步骤 | 内容 | 失败的影响 |
|---|---|---|
| 1 | 检查 Apple Silicon、工具、main 分支、干净工作区；版本号须高于线上 `stable.json`；试签验证私钥密码 | 无改动 |
| 2 | 写入 `Cargo.toml` 的 `[workspace.package] version` 并本地提交 `chore(release): v<版本>` | 只有本地提交；重跑会跳过 |
| 3 | `scripts/gate.sh` 与 `cargo test --workspace --exclude buddy-markdown` | 无上传 |
| 4 | `cargo build --release --locked`（`MACOSX_DEPLOYMENT_TARGET=12.0`） | 无上传 |
| 5 | `bundle-macos.sh` 组装 `Buddy.app`（ad-hoc 签名）、更新包（tar.xz）、DMG（ULMO）；`git archive` 源码包；签名；生成清单 | 无上传 |
| 6 | 上传版本目录，并从公网下载回来比对 sha256 | 版本目录存在但未被引用，用户不可见 |
| 7 | 发布确认（`--yes` 跳过） | — |
| 8 | 覆盖 `channels/stable.json` 并公网回读比对 | **此刻起用户可见** |
| 9 | 打本地标签 `v<版本>`（不推送） | — |

其他选项：`--skip-tests`（仅重跑上传 / 发布时使用）、`--skip-publish`（只上传版本目录）。
`--republish`：以线上同一版本号重新发布（覆盖该版本目录的制品、移动本地标签）。只用于还没有用户安装该版本时修正首发：
已安装同版本的客户端不会收到（版本没有变高）；标签已推送到远端时脚本拒绝执行。0.1.0 于 2026-10-04 用它重发过一次。
本地制品与日志在 `.release/<版本>/`（已被 `.gitignore` 忽略）。

## 5. 凭据

| 凭据 | 位置 | 说明 |
|---|---|---|
| 更新签名私钥 | `~/.tauri/buddy-v2.key`（权限 600） | 公钥内置于 `crates/update/src/lib.rs` 的 `PUBLIC_KEY`；**丢失后已发布客户端无法再验证任何新包**，私钥与密码须离线备份 |
| 私钥密码 | 钥匙串 `buddy-updater-key` / 账户 `buddy` | 脚本按「环境变量 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` → 钥匙串 → 终端输入」读取 |
| OSS AccessKey | `~/.ossutilconfig`（ossutil 2.x，region cn-beijing） | 需要 `buddy-release/buddy/*` 的 `GetObject`、`PutObject` |

不得读取、打印或提交上述内容。

## 6. 客户端行为

- 设置页「软件更新」：显示当前版本；点击「检查更新」→ 已是最新 / 发现新版本（版本号 + 更新说明 + 「立即更新」）/ 失败（中文原因 + 重新检查）。
- 以安装包运行时，启动 20 秒后与每 6 小时后台检查一次；只在设置页展示新版本，不弹窗、不自动下载。开发构建不做后台检查。
- 「立即更新」：下载到 `~/Library/Caches/com.buddy.chat/updates/`，大小、sha256、签名全部通过后解压，
  校验 bundle id / 版本 / 可执行文件 / 代码签名，再在原目录内原子替换（失败恢复原版本；目录不可写时请求管理员授权）。
- 正在流式回复时，等回复结束再重启；重启由独立进程在旧进程退出后 `open` 新应用。
- 下次启动清理旧应用备份与下载缓存。
- 从 DMG 直接运行或未移入「应用程序」的隔离副本会提示先移动到「应用程序」。

## 7. 故障与回滚

- 覆盖 `stable.json` 之前失败：修复后重跑，用户不受影响。
- 发布后发现问题：**不要把 `stable.json` 改回旧版本**（已升级用户不会降级）；修复后发布更高的修订版本。
- 不删除任何已发布版本目录。

## 8. 尚未完成

| 项 | spec |
|---|---|
| Developer ID 签名与公证：用户决定不做（S08-08 dropped）。当前 ad-hoc 签名，新用户首次打开 DMG 中的应用会被 Gatekeeper 拦截，需右键「打开」或在系统设置中允许 | — |
| Windows 安装包与安装流程（暂不处理） | S08-04 / S08-07 |
