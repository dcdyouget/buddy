#!/usr/bin/env node
/**
 * Prepare a trusted CI release from a workflow_dispatch run.
 *
 * This deliberately does not build, sign, or publish artifacts.  Its only
 * side effects are the version commit and annotated tag on main.  Keeping that
 * boundary small makes it safe for the later jobs to build an exact, public
 * source commit.
 */
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync, writeFileSync } from "node:fs";

const version = process.env.RELEASE_VERSION?.trim().replace(/^v/, "");
const notes = process.env.RELEASE_NOTES;
const output = process.env.GITHUB_OUTPUT;
const stableUrl = process.env.STABLE_MANIFEST_URL
  ?? "https://buddy-release.oss-cn-beijing.aliyuncs.com/buddy/channels/stable.json";

function fail(message) {
  throw new Error(message);
}

function run(command, args, options = {}) {
  return execFileSync(command, args, {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    ...options,
  }).trim();
}

function tryRun(command, args) {
  try {
    return { ok: true, stdout: run(command, args) };
  } catch (error) {
    return { ok: false, stdout: error.stdout?.toString().trim() ?? "", stderr: error.stderr?.toString().trim() ?? "" };
  }
}

function validVersion(value) {
  // The updater accepts stable releases only.  This also rejects shell-like
  // input before it reaches any external command.
  return /^(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)\.(?:0|[1-9]\d*)$/.test(value ?? "");
}

function compareVersions(a, b) {
  const left = a.split(".").map(BigInt);
  const right = b.split(".").map(BigInt);
  for (let index = 0; index < 3; index += 1) {
    if (left[index] !== right[index]) return left[index] > right[index] ? 1 : -1;
  }
  return 0;
}

function workspaceVersion(cargo) {
  const match = cargo.match(/(\[workspace\.package\][\s\S]*?\nversion\s*=\s*")(.*?)(")/);
  if (!match) fail("Cargo.toml is missing [workspace.package].version");
  return match[2];
}

function setWorkspaceVersion(cargo, value) {
  return cargo.replace(/(\[workspace\.package\][\s\S]*?\nversion\s*=\s*")(.*?)(")/, `$1${value}$3`);
}

function workspacePackageNames() {
  const manifests = [
    "apps/buddy/Cargo.toml",
    "crates/engine/Cargo.toml",
    "crates/markdown/Cargo.toml",
    "crates/markdown/util-shim/Cargo.toml",
    "crates/syntax/Cargo.toml",
    "crates/ui/Cargo.toml",
    "crates/update/Cargo.toml",
  ];
  return manifests.map((manifest) => {
    const content = readFileSync(manifest, "utf8");
    if (!/^version\.workspace\s*=\s*true\s*$/m.test(content)) {
      fail(`${manifest} no longer inherits workspace.package.version`);
    }
    const name = content.match(/^name\s*=\s*"([^"]+)"\s*$/m)?.[1];
    if (!name) fail(`Could not find package name in ${manifest}`);
    return name;
  });
}

function setLockfileWorkspaceVersions(lock, oldVersion, newVersion, names) {
  const nameSet = new Set(names);
  let changed = 0;
  let matched = 0;
  const result = lock.replace(/\[\[package\]\][\s\S]*?(?=\n\[\[package\]\]|$)/g, (entry) => {
    const name = entry.match(/^name\s*=\s*"([^"]+)"\s*$/m)?.[1];
    if (!nameSet.has(name)) return entry;
    matched += 1;
    const current = entry.match(/^version\s*=\s*"([^"]+)"\s*$/m)?.[1];
    if (current !== oldVersion && current !== newVersion) {
      fail(`Cargo.lock package ${name} has unexpected version ${current ?? "<missing>"}`);
    }
    if (current === newVersion) return entry;
    changed += 1;
    return entry.replace(/^(version\s*=\s*")[^"]+("\s*)$/m, `$1${newVersion}$2`);
  });
  if (matched !== names.length) {
    fail(`Cargo.lock should contain ${names.length} workspace packages; found ${matched}`);
  }
  return { lock: result, changed };
}

function versionAt(ref, file) {
  return run("git", ["show", `${ref}:${file}`]);
}

function packageLockMatchesWorkspace(lock, workspace) {
  if (lock.version !== undefined && lock.version !== workspace) return false;
  const root = lock.packages?.[""];
  return Boolean(root) && (root.version === undefined || root.version === workspace);
}

function assertTaggedSource(tag, expectedVersion, mainHead) {
  const commit = run("git", ["rev-parse", `${tag}^{}`]);
  if (commit !== mainHead) {
    const parent = run("git", ["rev-parse", `${commit}^`]);
    if (parent !== mainHead) {
      fail(`${tag} must be the dispatched main commit or its direct release child`);
    }
    const files = run("git", ["diff", "--name-only", mainHead, commit]).split("\n").filter(Boolean).sort();
    const versionFiles = new Set(["Cargo.lock", "Cargo.toml", "package-lock.json", "package.json"]);
    if (files.length === 0 || files.some((file) => !versionFiles.has(file))) {
      fail(`${tag} is not a pure version commit for the dispatched main source`);
    }
  }
  const taggedCargo = versionAt(tag, "Cargo.toml");
  const taggedPackage = JSON.parse(versionAt(tag, "package.json"));
  const taggedPackageLock = JSON.parse(versionAt(tag, "package-lock.json"));
  if (workspaceVersion(taggedCargo) !== expectedVersion || taggedPackage.version !== expectedVersion
    || taggedPackageLock.version !== expectedVersion || taggedPackageLock.packages?.[""]?.version !== expectedVersion) {
    fail(`${tag} does not contain matching Cargo.toml, package.json, and package-lock.json versions`);
  }
  return commit;
}

function writeOutputs(values) {
  if (!output) return;
  const delimiter = `BUDDY_${Date.now()}_${Math.random().toString(36).slice(2)}`;
  const content = Object.entries(values)
    .map(([key, value]) => `${key}<<${delimiter}\n${value}\n${delimiter}\n`)
    .join("");
  writeFileSync(output, content, { flag: "a" });
}

async function publishedVersion() {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 30_000);
  let response;
  try {
    response = await fetch(stableUrl, {
      headers: { "Cache-Control": "no-cache" },
      signal: controller.signal,
    });
  } catch (error) {
    if (error.name === "AbortError") fail("Timed out reading stable manifest after 30 seconds");
    throw error;
  } finally {
    clearTimeout(timeout);
  }
  if (response.status === 404) return undefined;
  if (!response.ok) fail(`Cannot read stable manifest (${response.status})`);
  const manifest = await response.json();
  if (!validVersion(manifest.version)) fail("stable manifest has an invalid version");
  return manifest.version;
}

async function main() {
  if (!validVersion(version)) fail("RELEASE_VERSION must be a stable SemVer, for example 0.2.0");
  if (typeof notes !== "string" || notes.trim().length === 0) fail("RELEASE_NOTES cannot be empty");
  if (!process.env.GH_TOKEN?.trim()) fail("GH_TOKEN is required for the release workflow");
  if (!output) fail("GITHUB_OUTPUT is required");
  if (!existsSync("Cargo.toml") || !existsSync("package.json") || !existsSync("package-lock.json") || !existsSync("Cargo.lock")) {
    fail("Run from the repository root after checkout");
  }
  if (run("git", ["status", "--porcelain"]) !== "") fail("Release checkout must be clean");

  const expectedHead = process.env.GITHUB_SHA;
  const mainHead = run("git", ["rev-parse", "HEAD"]);
  if (expectedHead && expectedHead !== mainHead) fail("Checked out source does not match GITHUB_SHA");
  if (run("git", ["branch", "--show-current"]) !== "main") fail("Release preparation must run on main");

  const online = await publishedVersion();
  if (online && compareVersions(version, online) <= 0) {
    fail(`Release version ${version} must exceed published version ${online}`);
  }

  // Fetch tags before deciding whether this is a retry.  The initial checkout
  // is intentionally pinned to github.sha, so it cannot silently pick up a
  // newer main commit while the release is being prepared.
  run("git", ["fetch", "--quiet", "origin", "main", "--tags"]);
  const tag = `v${version}`;
  const tagExists = tryRun("git", ["rev-parse", "--verify", `${tag}^{}`]).ok;
  let sourceCommit;
  if (tagExists) {
    sourceCommit = assertTaggedSource(tag, version, mainHead);
    console.log(`Reusing existing release tag ${tag} at ${sourceCommit}`);
  } else {
    const cargo = readFileSync("Cargo.toml", "utf8");
    const previousVersion = workspaceVersion(cargo);
    const packageJson = JSON.parse(readFileSync("package.json", "utf8"));
    const packageLock = JSON.parse(readFileSync("package-lock.json", "utf8"));
    if (packageJson.version !== undefined && !validVersion(packageJson.version)) {
      fail("package.json has an invalid version");
    }
    const nextCargo = setWorkspaceVersion(cargo, version);
    packageJson.version = version;
    const nextPackageJson = `${JSON.stringify(packageJson, null, 2)}\n`;
    if (!packageLock.packages?.[""]) fail("package-lock.json is missing its root package");
    if (!packageLockMatchesWorkspace(packageLock, previousVersion)) {
      fail("package-lock.json does not match the current workspace version");
    }
    packageLock.version = version;
    packageLock.packages[""].version = version;
    const nextPackageLock = `${JSON.stringify(packageLock, null, 2)}\n`;

    // A workspace-version change only alters the package stanzas in Cargo.lock.
    // Updating just those known local packages avoids a network resolution and
    // prevents a release from accidentally refreshing third-party dependencies.
    const names = workspacePackageNames();
    const lock = readFileSync("Cargo.lock", "utf8");
    const nextLock = setLockfileWorkspaceVersions(lock, previousVersion, version, names).lock;
    writeFileSync("Cargo.toml", nextCargo);
    writeFileSync("package.json", nextPackageJson);
    writeFileSync("package-lock.json", nextPackageLock);
    writeFileSync("Cargo.lock", nextLock);

    run("git", ["add", "Cargo.toml", "Cargo.lock", "package.json", "package-lock.json"]);
    if (tryRun("git", ["diff", "--cached", "--quiet"]).ok) {
      sourceCommit = mainHead;
    } else {
      run("git", ["commit", "-m", `chore(release): v${version}`]);
      sourceCommit = run("git", ["rev-parse", "HEAD"]);
    }
    run("git", ["tag", "-a", tag, "-m", `Buddy v${version}`, sourceCommit]);

    // The atomic push protects main from a partial release source: either the
    // version commit and its tag are public together, or neither is pushed.
    run("git", ["push", "--atomic", "origin", `HEAD:refs/heads/main`, `refs/tags/${tag}`]);
  }

  writeOutputs({ version, commit: sourceCommit, notes });
  console.log(`Prepared Buddy ${version} from ${sourceCommit}`);
}

main().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
