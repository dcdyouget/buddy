// Smoke test the release-source transaction against a disposable local Git
// remote.  It proves version, lockfile, tag, push, and retry semantics without
// touching GitHub or OSS.
import { execFileSync, spawn } from "node:child_process";
import { cpSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { createServer } from "node:http";

const root = resolve(import.meta.dirname, "../..");
const temp = mkdtempSync(join(tmpdir(), "buddy-release-prepare-"));
const remote = join(temp, "remote.git");
const repo = join(temp, "repo");
const script = join(root, "scripts/release/prepare-ci-release.mjs");
function run(command, args, cwd = temp) {
  return execFileSync(command, args, { cwd, encoding: "utf8", stdio: "pipe" }).trim();
}
function assert(condition, message) { if (!condition) throw new Error(message); }
function listen(server) { return new Promise((resolveListen) => server.listen(0, "127.0.0.1", resolveListen)); }

const server = createServer((_, response) => {
  response.setHeader("content-type", "application/json");
  response.end('{"version":"0.1.10"}');
});
try {
  await listen(server);
  const stableUrl = `http://127.0.0.1:${server.address().port}/stable.json`;
  run("git", ["init", "--bare", remote]);
  run("git", ["clone", remote, repo]);
  run("git", ["checkout", "-b", "main"], repo);
  run("git", ["config", "user.email", "test@example.invalid"], repo);
  run("git", ["config", "user.name", "Release test"], repo);
  for (const file of ["Cargo.toml", "Cargo.lock", "package.json", "package-lock.json", "apps", "crates"]) {
    cpSync(join(root, file), join(repo, file), { recursive: true });
  }
  run("git", ["add", "."], repo);
  run("git", ["commit", "-m", "initial"], repo);
  run("git", ["push", "-u", "origin", "main"], repo);
  const before = run("git", ["rev-parse", "HEAD"], repo);
  const output = join(temp, "outputs");
  const env = { ...process.env, RELEASE_VERSION: "0.1.11", RELEASE_NOTES: "release notes", GH_TOKEN: "test", GITHUB_OUTPUT: output, GITHUB_SHA: before, STABLE_MANIFEST_URL: stableUrl };
  const first = spawn(process.execPath, [script], { cwd: repo, env, stdio: "inherit" });
  const firstCode = await new Promise((resolveExit) => first.once("exit", resolveExit));
  assert(firstCode === 0, "first preparation failed");
  const commit = run("git", ["rev-parse", "v0.1.11^{}"], repo);
  assert(commit !== before, "release commit was not created");
  assert(readFileSync(join(repo, "package.json"), "utf8").includes('"version": "0.1.11"'), "package.json version missing");
  assert(readFileSync(join(repo, "package-lock.json"), "utf8").includes('"version": "0.1.11"'), "package-lock.json version missing");
  assert(readFileSync(join(repo, "Cargo.lock"), "utf8").includes('version = "0.1.11"'), "Cargo.lock version missing");
  assert(readFileSync(output, "utf8").includes(`commit<<`) && readFileSync(output, "utf8").includes(commit), "outputs missing commit");
  // GitHub's “re-run all jobs” keeps the original event SHA.  A fresh
  // checkout therefore sees the pre-release commit and the public tag as its
  // direct child, which must be accepted without moving the tag.
  run("git", ["checkout", "-B", "main", before], repo);
  const retry = spawn(process.execPath, [script], { cwd: repo, env, stdio: "inherit" });
  const retryCode = await new Promise((resolveExit) => retry.once("exit", resolveExit));
  assert(retryCode === 0, "retry should reuse the existing tag");
  const lower = spawn(process.execPath, [script], { cwd: repo, env: { ...env, RELEASE_VERSION: "0.1.10" }, stdio: "ignore" });
  const lowerCode = await new Promise((resolveExit) => lower.once("exit", resolveExit));
  assert(lowerCode !== 0, "published version must be rejected");
  const racing = spawn(process.execPath, [script], { cwd: repo, env: { ...env, RELEASE_VERSION: "0.1.12" }, stdio: "ignore" });
  const racingCode = await new Promise((resolveExit) => racing.once("exit", resolveExit));
  assert(racingCode !== 0, "atomic push must fail when remote main advanced");
  assert(run("git", ["ls-remote", "--tags", "origin", "v0.1.12"], repo) === "", "failed atomic push leaked a tag");
  console.log("prepare-ci-release tests passed");
} finally {
  server.close();
  rmSync(temp, { recursive: true, force: true });
}
