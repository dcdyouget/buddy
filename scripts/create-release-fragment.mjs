import { readFileSync, writeFileSync } from "node:fs";

function parseArgs(argv) {
  const result = {};
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index];
    const value = argv[index + 1];
    if (!key?.startsWith("--") || value === undefined) {
      throw new Error(`无效参数：${key ?? "<空>"}`);
    }
    result[key.slice(2)] = value;
  }
  return result;
}

const args = parseArgs(process.argv.slice(2));
const required = ["version", "platform", "url", "signature-file", "output"];

for (const key of required) {
  if (!args[key]) {
    throw new Error(`缺少参数：--${key}`);
  }
}

const signature = readFileSync(args["signature-file"], "utf8").trim();
if (!signature) {
  throw new Error(`签名文件为空：${args["signature-file"]}`);
}

writeFileSync(
  args.output,
  `${JSON.stringify(
    {
      version: args.version,
      platform: args.platform,
      url: args.url,
      signature,
    },
    null,
    2,
  )}\n`,
);
