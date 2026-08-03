import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const version = process.argv[2]?.replace(/^v/, "");
if (!version || !/^[0-9]+\.[0-9]+\.[0-9]+(?:[+-][0-9A-Za-z.-]+)?$/.test(version)) {
  throw new Error("用法：npm run version:set -- <SemVer>，例如 1.4.0");
}

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const projectRoot = path.resolve(scriptDir, "..");

function updateJson(relativePath, update) {
  const filePath = path.join(projectRoot, relativePath);
  const value = JSON.parse(readFileSync(filePath, "utf8"));
  update(value);
  writeFileSync(filePath, `${JSON.stringify(value, null, 2)}\n`);
}

updateJson("package.json", (value) => {
  value.version = version;
});

updateJson("package-lock.json", (value) => {
  value.version = version;
  if (value.packages?.[""]) {
    value.packages[""].version = version;
  }
});

updateJson("src-tauri/tauri.conf.json", (value) => {
  value.version = version;
});

const cargoPath = path.join(projectRoot, "src-tauri/Cargo.toml");
const cargo = readFileSync(cargoPath, "utf8");
const cargoVersionPattern = /(\[package\][\s\S]*?\nversion\s*=\s*")[^"]+("\s*\n)/;
if (!cargoVersionPattern.test(cargo)) {
  throw new Error("未能更新 src-tauri/Cargo.toml 的 package.version");
}
const updatedCargo = cargo.replace(cargoVersionPattern, `$1${version}$2`);
writeFileSync(cargoPath, updatedCargo);

console.log(`版本已更新为 ${version}`);
console.log("请补充更新日志，提交改动并创建标签后再执行发布脚本。");
