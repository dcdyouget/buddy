import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { spawnSync } from "node:child_process";

function fixture(run) {
  const dir = mkdtempSync(path.join(tmpdir(), "buddy-manifest-"));
  try { run(dir); } finally { rmSync(dir, { recursive: true, force: true }); }
}
function assets(dir, platform) {
  const base = platform === "mac" ? "macos/aarch64/Buddy_0.2.0_aarch64" : "windows/x86_64/Buddy_0.2.0_x86_64";
  mkdirSync(path.dirname(path.join(dir, base)), { recursive: true });
  for (const extension of platform === "mac" ? ["app.tar.xz", "dmg"] : ["exe", "zip"]) {
    writeFileSync(path.join(dir, `${base}.${extension}`), `test-${extension}`);
    writeFileSync(path.join(dir, `${base}.${extension}.sig`), "test-signature");
  }
}
function generate(dir, extra = []) {
  writeFileSync(path.join(dir, "notes.txt"), "支持 Windows");
  return spawnSync(process.execPath, ["scripts/release/manifest.mjs", "--version", "0.2.0", "--notes-file", path.join(dir, "notes.txt"),
    "--base-url", "https://example.com/0.2.0", "--source-url", "https://example.com/source", "--dir", dir,
    "--output", path.join(dir, "manifest.json"), ...extra], { encoding: "utf8" });
}
test("同一清单包含 Mac 和 Windows 各自的更新包及安装包", () => fixture((dir) => {
  assets(dir, "mac"); assets(dir, "windows");
  const result = generate(dir);
  assert.equal(result.status, 0, result.stderr);
  const manifest = JSON.parse(readFileSync(path.join(dir, "manifest.json")));
  assert.deepEqual(Object.keys(manifest.platforms), ["darwin-aarch64", "windows-x86_64"]);
  assert.match(manifest.platforms["windows-x86_64"].update.url, /\.exe$/);
  assert.match(manifest.platforms["windows-x86_64"].installer.url, /\.zip$/);
  assert.equal(manifest.platforms["windows-x86_64"].update.sha256.length, 64);
}));
test("分机器构建可以合并同版本平台清单", () => fixture((dir) => {
  assets(dir, "mac"); assert.equal(generate(dir).status, 0);
  const merge = path.join(dir, "mac-manifest.json");
  writeFileSync(merge, readFileSync(path.join(dir, "manifest.json")));
  assets(dir, "windows");
  const result = generate(dir, ["--platforms", "windows-x86_64", "--merge", merge]);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(Object.keys(JSON.parse(readFileSync(path.join(dir, "manifest.json"))).platforms).length, 2);
}));
test("拒绝合并旧版本包或发布缺少签名的包", () => fixture((dir) => {
  assets(dir, "windows");
  const merge = path.join(dir, "old.json");
  writeFileSync(merge, JSON.stringify({ schema: 1, version: "0.1.8", platforms: {} }));
  assert.notEqual(generate(dir, ["--merge", merge]).status, 0);
  rmSync(path.join(dir, "windows/x86_64/Buddy_0.2.0_x86_64.exe.sig"));
  assert.notEqual(generate(dir).status, 0);
}));
test("拒绝同版本清单中的损坏资产及空更新包", () => fixture((dir) => {
  assets(dir, "windows");
  const merge = path.join(dir, "invalid.json");
  writeFileSync(merge, JSON.stringify({ schema: 1, version: "0.2.0", platforms: { "darwin-aarch64": {} } }));
  assert.notEqual(generate(dir, ["--merge", merge]).status, 0);
  writeFileSync(path.join(dir, "windows/x86_64/Buddy_0.2.0_x86_64.exe"), "");
  assert.notEqual(generate(dir).status, 0);
}));
