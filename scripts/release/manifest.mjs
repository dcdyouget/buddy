// 生成版本清单（客户端 crates/update/src/manifest.rs 解析同一格式；schema 变更须两边同步）。
//
// 用法：
//   node scripts/release/manifest.mjs --version 0.1.0 --notes-file notes.txt \
//     --base-url https://.../buddy/releases/0.1.0 --dir .release/0.1.0 --output manifest.json
//
// --dir 下需要：macos/aarch64/Buddy_<v>_aarch64.{app.tar.xz,dmg} 及对应 .sig、source/buddy-<v>-src.tar.gz
import { createHash } from "node:crypto";
import { readFileSync, statSync, writeFileSync } from "node:fs";
import path from "node:path";

const args = {};
const argv = process.argv.slice(2);
for (let i = 0; i < argv.length; i += 2) {
  if (!argv[i].startsWith("--") || argv[i + 1] === undefined) {
    throw new Error(`无效参数：${argv[i]}`);
  }
  args[argv[i].slice(2)] = argv[i + 1];
}
for (const key of ["version", "notes-file", "base-url", "dir", "output"]) {
  if (!args[key]) throw new Error(`缺少参数：--${key}`);
}

const { version } = args;
const notes = readFileSync(args["notes-file"], "utf8").trim();
if (!notes) throw new Error("更新说明不能为空");
if (!args["base-url"].startsWith("https://")) throw new Error("base-url 必须是 HTTPS");

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
const manifest = {
  schema: 1,
  version,
  pub_date: new Date().toISOString().replace(/\.\d{3}Z$/, "Z"),
  notes,
  platforms: {
    "darwin-aarch64": {
      update: asset(`${mac}.app.tar.xz`, true),
      installer: asset(`${mac}.dmg`, true),
    },
  },
  // GPL-3.0：每个发布版本提供对应源码
  source: asset(`source/buddy-${version}-src.tar.gz`, false),
};

writeFileSync(args.output, `${JSON.stringify(manifest, null, 2)}\n`);
console.log(`清单已生成：${args.output}`);
