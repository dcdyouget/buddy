// 设置 v2（GPUI）版本号：根 Cargo.toml 的 [workspace.package] version。
// 所有 crate 通过 `version.workspace = true` 继承；客户端用它判断是否有更新，
// 发布脚本用它生成 Info.plist 与清单版本，三者必须一致。
//
// 用法：npm run version:set -- <SemVer>，例如 0.1.0
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const version = process.argv[2]?.replace(/^v/, "");
if (!version || !/^[0-9]+\.[0-9]+\.[0-9]+$/.test(version)) {
  throw new Error("用法：npm run version:set -- <主.次.修订>，例如 0.1.0");
}

const projectRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cargoPath = path.join(projectRoot, "Cargo.toml");
const cargo = readFileSync(cargoPath, "utf8");
const pattern = /(\[workspace\.package\][^[]*?\nversion\s*=\s*")[^"]+(")/;
if (!pattern.test(cargo)) {
  throw new Error("未找到 Cargo.toml 的 [workspace.package] version");
}
writeFileSync(cargoPath, cargo.replace(pattern, `$1${version}$2`));
console.log(`版本已更新为 ${version}（Cargo.lock 由下一次 cargo 命令同步）`);
