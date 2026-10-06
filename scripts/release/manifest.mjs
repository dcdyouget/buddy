// 生成版本清单（客户端 crates/update/src/manifest.rs 解析同一格式；schema 变更须两边同步）。
//
// 用法：
//   node scripts/release/manifest.mjs --version 0.2.0 --notes-file notes.txt \
//     --base-url https://.../buddy/releases/0.2.0 --source-url https://github.com/.../tree/v0.2.0 \
//     --dir .release/0.2.0 --output manifest.json
//
// --dir 下需要：macos/aarch64/Buddy_<v>_aarch64.{app.tar.xz,dmg} 或
// windows/x86_64/Buddy_<v>_x86_64.{exe,zip} 及对应 .sig。
// --platforms 可显式指定平台；--merge 只合并同版本清单。
import { createHash } from "node:crypto";
import { existsSync, readFileSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";

const args = {};
const argv = process.argv.slice(2);
for (let i = 0; i < argv.length; i += 2) {
  if (!argv[i].startsWith("--") || argv[i + 1] === undefined) {
    throw new Error(`无效参数：${argv[i]}`);
  }
  args[argv[i].slice(2)] = argv[i + 1];
}
for (const key of ["version", "notes-file", "base-url", "source-url", "dir", "output"]) {
  if (!args[key]) throw new Error(`缺少参数：--${key}`);
}

const { version } = args;
const notes = readFileSync(args["notes-file"], "utf8").trim();
if (!notes) throw new Error("更新说明不能为空");
if (!/^\d+\.\d+\.\d+$/.test(version)) throw new Error("版本号必须是 主.次.修订");
for (const key of ["base-url", "source-url"]) {
  const url = new URL(args[key]);
  if (url.protocol !== "https:" || url.username || url.password) throw new Error(`${key} 必须是 HTTPS`);
}

function asset(relative, signed) {
  const file = path.join(args.dir, relative);
  const data = readFileSync(file);
  const entry = {
    url: `${args["base-url"]}/${relative}`,
    size: statSync(file).size,
    sha256: createHash("sha256").update(data).digest("hex"),
  };
  if (signed) {
    entry.signature = readFileSync(`${file}.sig`, "utf8").trim();
    if (!entry.signature) throw new Error(`签名为空：${file}.sig`);
  }
  return entry;
}

const mac = `macos/aarch64/Buddy_${version}_aarch64`;
const windows = `windows/x86_64/Buddy_${version}_x86_64`;
// 同一版本可以分两台机器构建；只合并版本完全一致的清单，不能把旧包标为新版本。
const merged = args.merge ? JSON.parse(readFileSync(args.merge, "utf8")) : null;
if (merged && (merged.schema !== 1 || merged.version !== version)) {
  throw new Error("合并清单必须使用 schema 1 且版本号完全一致");
}
if (merged && (!merged.platforms || Array.isArray(merged.platforms) || typeof merged.platforms !== "object")) {
  throw new Error("合并清单缺少有效的平台制品");
}
const platforms = { ...merged?.platforms };
const requested = args.platforms?.split(",") ?? ["darwin-aarch64", "windows-x86_64"].filter((platform) =>
  existsSync(path.join(args.dir, platform === "darwin-aarch64" ? `${mac}.app.tar.xz` : `${windows}.exe`)));
for (const platform of requested) {
  if (platform === "darwin-aarch64") {
    platforms[platform] = { update: asset(`${mac}.app.tar.xz`, true), installer: asset(`${mac}.dmg`, true) };
  } else if (platform === "windows-x86_64") {
    platforms[platform] = { update: asset(`${windows}.exe`, true), installer: asset(`${windows}.zip`, true) };
  } else {
    throw new Error(`不支持的平台：${platform}`);
  }
}
if (!requested.length) throw new Error("没有找到本次构建的制品");
for (const [platform, release] of Object.entries(platforms)) {
  for (const name of ["update", "installer"]) {
    const entry = release?.[name];
    if (!entry || !entry.url?.startsWith("https://") || !Number.isSafeInteger(entry.size) || entry.size <= 0 ||
        !/^[0-9a-f]{64}$/i.test(entry.sha256 ?? "") || typeof entry.signature !== "string" || !entry.signature.trim()) {
      throw new Error(`${platform}.${name} 制品的地址、大小、哈希或签名无效`);
    }
  }
}
const manifest = {
  schema: 1,
  version,
  pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
  notes,
  platforms,
  // GPL-3.0：每个发布版本提供对应源码（GitHub 上的版本标签）
  source: { url: args["source-url"] },
};

writeFileSync(args.output, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`清单已生成：${args.output}`);
