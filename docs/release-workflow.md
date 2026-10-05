# Buddy 发布与升级

适用：macOS Apple Silicon（`darwin-aarch64`）。

## 固定地址

| 用途 | 地址 |
|---|---|
| 版本清单（客户端检查更新；产品介绍页读取最新安装包地址） | `https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/channels/stable.json` |
| 版本制品 | `https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/releases/<版本>/...` |

清单地址写死在已发布的客户端里（`crates/update/src/lib.rs` 的 `MANIFEST_URL`）：
bucket `buddy-release`（cn-beijing）不能删除、改名或换地域。换地址只能在新版本客户端中进行，并让旧地址继续发布到旧版本用户都升级为止。

产品介绍页的「下载最新版」读 `platforms["darwin-aarch64"].installer.url`，源码链接读 `source.url`（GPL 要求提供源码）。

## 清单格式（`schema: 1`）

```json
{
  "schema": 1,
  "version": "0.1.2",
  "pub_date": "2026-10-05T00:00:00Z",
  "notes": "中文更新说明",
  "platforms": {
    "darwin-aarch64": {
      "update":    { "url": ".../Buddy_0.1.2_aarch64.app.tar.xz", "size": 0, "sha256": "…", "signature": "…" },
      "installer": { "url": ".../Buddy_0.1.2_aarch64.dmg",        "size": 0, "sha256": "…", "signature": "…" }
    }
  },
  "source": { "url": ".../source/buddy-0.1.2-src.tar.gz", "size": 0, "sha256": "…" }
}
```

由 `scripts/release/manifest.mjs` 生成、`crates/update/src/manifest.rs` 解析；不兼容的格式变更必须提升 `schema`。

## OSS 目录

```
buddy/
  channels/stable.json                                  ← 发布开关，唯一会被覆盖的对象
  releases/<版本>/
    manifest.json
    macos/aarch64/Buddy_<版本>_aarch64.app.tar.xz(.sig)   ← 应用内更新包
    macos/aarch64/Buddy_<版本>_aarch64.dmg(.sig)          ← 安装包
    source/buddy-<版本>-src.tar.gz                        ← 源码
```

所有对象使用 `Cache-Control: no-cache`（浏览器向 OSS 校验 ETag，未变化时 304）。

## 发版

```bash
npm run release                                      # 交互输入版本号与更新说明
npm run release -- 0.2.0 --notes "修复 xxx；新增 yyy"
npm run release -- 0.2.0 --notes-file notes.txt --yes
```

`scripts/release/release.sh` 依次：环境与版本检查（新版本须高于线上）→ 写版本号并本地提交 → `scripts/gate.sh` 与全部测试 →
release 构建 → 组装 `Buddy.app`、更新包、DMG、源码包并签名 → 上传版本目录并公网回读校验 → 确认 → 覆盖 `channels/stable.json` → 打本地标签。

只有覆盖 `stable.json` 这一步让用户看到新版本；之前任何一步失败都不影响用户，修复后重跑即可。

其他选项：`--skip-tests`（重跑上传 / 发布时）、`--skip-publish`（只上传版本目录）、
`--republish`（以线上同一版本号重发：只用于没人安装过该版本时修正首发，已安装同版本的客户端收不到；标签已推送时拒绝）。

## 凭据

| 凭据 | 位置 |
|---|---|
| 更新签名私钥 | `~/.tauri/buddy-v2.key`（公钥内置于 `crates/update/src/lib.rs` 的 `PUBLIC_KEY`）。**丢失后已发布客户端无法再验证任何新包**，私钥与密码须离线备份 |
| 私钥密码 | 钥匙串 `buddy-updater-key` / 账户 `buddy`（或环境变量 `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`） |
| OSS AccessKey | `~/.ossutilconfig`（ossutil 2.x），需要 `buddy-release/buddy/*` 的 `GetObject`、`PutObject` |

## 出问题时

- 发布后发现问题：不要把 `stable.json` 改回旧版本（已升级的用户不会降级），修复后发布更高的修订版本。
- 不删除已发布的版本目录。
- 安装包为 ad-hoc 签名：新用户首次打开会被 Gatekeeper 拦截，需右键「打开」或在系统设置中允许。
