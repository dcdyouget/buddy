#!/usr/bin/env node
// Publish the signed artifacts produced by the trusted GitHub release workflow.
// The OSS V1 request signer is intentionally implemented here rather than
// downloading ossutil during a release: GitHub Actions only needs Node's built
// in crypto and the two scoped OSS credentials.
import { createHash, createHmac, randomUUID } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { uploadMultipart } from "./oss-multipart.mjs";

const BUCKET = "buddy-release";
const REGION_ENDPOINT = "https://buddy-release.oss-cn-beijing.aliyuncs.com";
const PUBLIC_BASE = "https://buddy-release.oss-cn-beijing.aliyuncs.com";
const REPOSITORY = "dcdyouget/buddy";
// Hosted runners upload to an OSS region in China; allow complete installers
// to cross that route without treating a slow transfer as a failed request.
const FETCH_TIMEOUT_MS = 600_000;

function sha256(data) {
  return createHash("sha256").update(data).digest("hex");
}

export function canonicalOssResource(bucket, key) {
  if (!bucket || !key || key.startsWith("/") || key.split("/").some((part) => !part || part === "." || part === "..")) {
    throw new Error("Invalid OSS object key");
  }
  return `/${bucket}/${key}`;
}

export function ossStringToSign({ method, contentType, date, bucket, key, ossHeaders = {} }) {
  const canonicalHeaders = Object.entries(ossHeaders)
    .map(([name, value]) => [name.toLowerCase(), String(value).trim()])
    .filter(([name]) => name.startsWith("x-oss-"))
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([name, value]) => `${name}:${value}\n`)
    .join("");
  return `${method}\n\n${contentType}\n${date}\n${canonicalHeaders}${canonicalOssResource(bucket, key)}`;
}

export function ossAuthorization({ accessKeyId, accessKeySecret, method, contentType, date, bucket, key, ossHeaders }) {
  if (!accessKeyId || !accessKeySecret) throw new Error("OSS_ACCESS_KEY_ID and OSS_ACCESS_KEY_SECRET are required");
  const stringToSign = ossStringToSign({ method, contentType, date, bucket, key, ossHeaders });
  const signature = createHmac("sha1", accessKeySecret).update(stringToSign).digest("base64");
  return `OSS ${accessKeyId}:${signature}`;
}

function requiredEnv(name) {
  const value = process.env[name];
  if (!value) throw new Error(`${name} is required`);
  return value;
}

function run(command, args, options = {}) {
  try {
    return execFileSync(command, args, { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"], ...options }).trim();
  } catch (error) {
    const stderr = error.stderr?.toString().trim();
    throw new Error(`${command} ${args[0] ?? ""} failed${stderr ? `: ${stderr}` : ""}`);
  }
}

function runVisible(command, args) {
  try {
    execFileSync(command, args, { stdio: "inherit" });
  } catch (error) {
    throw new Error(`${command} ${args[0] ?? ""} failed with exit code ${error.status ?? "unknown"}`);
  }
}

function versionParts(version) {
  const matched = /^(\d+)\.(\d+)\.(\d+)$/.exec(version);
  if (!matched) throw new Error("RELEASE_VERSION must have the form major.minor.patch");
  return matched.slice(1).map((part) => Number(part));
}

export function compareVersions(left, right) {
  const leftParts = versionParts(left);
  const rightParts = versionParts(right);
  for (let index = 0; index < leftParts.length; index += 1) {
    if (leftParts[index] !== rightParts[index]) return leftParts[index] > rightParts[index] ? 1 : -1;
  }
  return 0;
}

function publicUrl(key) {
  return `${PUBLIC_BASE}/${key.split("/").map(encodeURIComponent).join("/")}`;
}

function ossUrl(key) {
  return `${REGION_ENDPOINT}/${key.split("/").map(encodeURIComponent).join("/")}`;
}

export async function fetchWithTimeout(url, init, { attempts = 1 } = {}) {
  let lastError;
  for (let attempt = 1; attempt <= attempts; attempt += 1) {
    try {
      const response = await fetch(url, { ...init, signal: AbortSignal.timeout(FETCH_TIMEOUT_MS) });
      if (attempt < attempts && (response.status === 429 || response.status >= 500)) {
        await response.body?.cancel();
        await new Promise((resolve) => setTimeout(resolve, attempt * 500));
        continue;
      }
      return response;
    } catch (error) {
      lastError = error;
      if (attempt < attempts) {
        await new Promise((resolve) => setTimeout(resolve, attempt * 500));
      }
    }
  }
  const cause = /^[A-Z0-9_]+$/.test(lastError?.cause?.code ?? "") ? ` (${lastError.cause.code})` : "";
  const reason = lastError?.name === "TimeoutError" ? `timed out after ${FETCH_TIMEOUT_MS / 1000}s` : `${lastError?.message ?? "unknown error"}${cause}`;
  throw new Error(`Network request failed: ${reason ?? "unknown error"}`);
}

function createOssClient() {
  const accessKeyId = requiredEnv("OSS_ACCESS_KEY_ID").trim();
  const accessKeySecret = requiredEnv("OSS_ACCESS_KEY_SECRET").trim();
  async function request(method, key, body, contentType = "application/octet-stream", attempts = 1) {
    if (method === "PUT" && Buffer.isBuffer(body) && body.length > 8 * 1024 * 1024) {
      await uploadMultipart({
        bucket: BUCKET, endpoint: REGION_ENDPOINT, accessKeyId, accessKeySecret,
        key, data: body, contentType, timeoutMs: FETCH_TIMEOUT_MS,
      });
      return;
    }
    const date = new Date().toUTCString();
    const authorization = ossAuthorization({ accessKeyId, accessKeySecret, method, contentType, date, bucket: BUCKET, key });
    const response = await fetchWithTimeout(ossUrl(key), {
      method,
      headers: {
        Authorization: authorization,
        Date: date,
        "Content-Type": contentType,
        "Cache-Control": "no-cache",
      },
      body,
    }, { attempts });
    if (!response.ok) {
      // OSS error bodies may contain credential identifiers or signing data.
      // Only expose the bounded error code and request ID for diagnosis.
      const body = await response.text();
      const code = /<Code>([A-Za-z0-9_-]{1,80})<\/Code>/.exec(body)?.[1] ?? "Unknown";
      const requestId = /<RequestId>([A-Za-z0-9_-]{1,100})<\/RequestId>/.exec(body)?.[1] ?? "unknown";
      throw new Error(`OSS ${method} failed for ${key}: HTTP ${response.status}, code ${code}, request ${requestId}`);
    }
    return response;
  }
  return { request };
}

async function fetchPublic(key, { attempts = 3 } = {}) {
  const response = await fetchWithTimeout(publicUrl(key), { headers: { "Cache-Control": "no-cache" }, cache: "no-store" }, { attempts });
  if (response.status === 404) return null;
  if (!response.ok) throw new Error(`Public OSS read failed for ${key}: HTTP ${response.status}`);
  return Buffer.from(await response.arrayBuffer());
}

async function putAndVerify(client, localFile, key, { immutable = false, contentType = "application/octet-stream", attempts = 1 } = {}) {
  const data = readFileSync(localFile);
  const expected = sha256(data);
  if (immutable) {
    const existing = await fetchPublic(key);
    if (existing) {
      if (sha256(existing) !== expected) {
        throw new Error(`Refusing to replace an existing release artifact: ${key}`);
      }
      console.log(`Already uploaded and verified: ${key}`);
      return;
    }
  }
  await client.request("PUT", key, data, contentType, attempts);
  const published = await fetchPublic(key);
  if (!published || sha256(published) !== expected) throw new Error(`Public checksum mismatch: ${key}`);
  console.log(`Uploaded and verified: ${key}`);
}

function workspaceVersion() {
  const cargoToml = readFileSync("Cargo.toml", "utf8");
  const section = /\[workspace\.package\]([\s\S]*?)(?:\n\[|$)/.exec(cargoToml)?.[1];
  const version = section && /^version\s*=\s*"(\d+\.\d+\.\d+)"\s*$/m.exec(section)?.[1];
  if (!version) throw new Error("Could not read workspace version from Cargo.toml");
  return version;
}

function assertSource(version, commit) {
  if (!/^[0-9a-f]{40}$/i.test(commit)) throw new Error("RELEASE_COMMIT must be a 40-character commit SHA");
  if (workspaceVersion() !== version) throw new Error("RELEASE_VERSION does not match Cargo.toml");
  if (run("git", ["rev-parse", "HEAD"]).toLowerCase() !== commit.toLowerCase()) {
    throw new Error("Current checkout does not match RELEASE_COMMIT");
  }
  if (run("git", ["rev-parse", `v${version}^{}`]).toLowerCase() !== commit.toLowerCase()) {
    throw new Error("Release tag does not resolve to RELEASE_COMMIT");
  }
  const remoteTags = run("git", ["ls-remote", "--exit-code", "--tags", "origin", `refs/tags/v${version}`, `refs/tags/v${version}^{}`]);
  if (!remoteTags.split(/\r?\n/).some((line) => line.toLowerCase().startsWith(`${commit.toLowerCase()}\t`))) {
    throw new Error("Release tag must be public before publishing");
  }
}

async function assertNewVersion(version) {
  const stable = await fetchPublic("buddy/channels/stable.json");
  if (!stable) return;
  let published;
  try {
    published = JSON.parse(stable.toString("utf8"));
  } catch {
    throw new Error("Published stable.json is not valid JSON");
  }
  if (typeof published.version !== "string") throw new Error("Published stable.json has no version");
  if (compareVersions(version, published.version) <= 0) {
    throw new Error(`RELEASE_VERSION must exceed the published version ${published.version}`);
  }
}

function hasSignedArtifact(dir, relative) {
  const file = path.join(dir, relative);
  return existsSync(file) && existsSync(`${file}.sig`) && statSync(file).size > 0 && statSync(`${file}.sig`).size > 0;
}

function artifactList(dir, version) {
  const files = [
    `macos/aarch64/Buddy_${version}_aarch64.app.tar.xz`,
    `macos/aarch64/Buddy_${version}_aarch64.dmg`,
    `windows/x86_64/Buddy_${version}_x86_64.exe`,
  ];
  const setup = `windows/x86_64/Buddy_${version}_x86_64_setup.exe`;
  const zip = `windows/x86_64/Buddy_${version}_x86_64.zip`;
  if (existsSync(path.join(dir, setup))) {
    files.push(setup);
    // Keep the portable download only when its signature is also available.
    if (hasSignedArtifact(dir, zip)) files.push(zip);
  } else {
    // A release resumed from artifacts created before setup installers uses ZIP.
    files.push(zip);
  }
  return files;
}

function requireArtifacts(dir, version) {
  const files = artifactList(dir, version);
  for (const relative of files) {
    for (const suffix of ["", ".sig"]) {
      const file = path.join(dir, `${relative}${suffix}`);
      if (!existsSync(file) || statSync(file).size === 0) throw new Error(`Missing or empty signed artifact: ${relative}${suffix}`);
    }
  }
  return files;
}

function verifyArtifacts(dir, files) {
  for (const relative of files) {
    const file = path.join(dir, relative);
    runVisible("cargo", ["run", "--locked", "-p", "buddy-update", "--example", "verify-artifact", "--", file, `${file}.sig`]);
  }
}

function writeNotes(dir) {
  const notes = requiredEnv("RELEASE_NOTES").trim();
  if (!notes) throw new Error("RELEASE_NOTES must not be empty");
  const notesFile = path.join(dir, "notes.txt");
  writeFileSync(notesFile, `${notes}\n`, "utf8");
  return notesFile;
}

function generateManifest(dir, version, notesFile) {
  const output = path.join(dir, "manifest.json");
  runVisible("node", ["scripts/release/manifest.mjs", "--version", version, "--notes-file", notesFile,
    "--base-url", `${PUBLIC_BASE}/buddy/releases/${version}`, "--source-url", `https://github.com/${REPOSITORY}/tree/v${version}`,
    "--dir", dir, "--output", output, "--platforms", "darwin-aarch64,windows-x86_64"]);
  return output;
}

function releaseExists(version) {
  try {
    run("gh", ["release", "view", `v${version}`, "--repo", REPOSITORY, "--json", "tagName"]);
    return true;
  } catch {
    return false;
  }
}

function sameGitHubAsset(version, filename, source) {
  const temporary = mkdtempSync(path.join(tmpdir(), "buddy-release-asset-"));
  try {
    runVisible("gh", ["release", "download", `v${version}`, "--repo", REPOSITORY, "--pattern", filename, "--dir", temporary]);
    const downloaded = path.join(temporary, filename);
    return existsSync(downloaded) && sha256(readFileSync(downloaded)) === sha256(readFileSync(source));
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
}

function releaseAssets(files, version) {
  const setup = `windows/x86_64/Buddy_${version}_x86_64_setup.exe`;
  const zip = `windows/x86_64/Buddy_${version}_x86_64.zip`;
  const selected = [`macos/aarch64/Buddy_${version}_aarch64.dmg`];
  if (files.includes(setup)) {
    selected.push(setup);
    if (files.includes(zip)) selected.push(zip);
  } else {
    selected.push(zip);
  }
  return selected;
}

function ensureGitHubRelease(version, notesFile, dir, files) {
  if (!releaseExists(version)) {
    runVisible("gh", ["release", "create", `v${version}`, "--repo", REPOSITORY, "--verify-tag", "--latest",
      "--title", `Buddy ${version}`, "--notes-file", notesFile]);
  } else {
    runVisible("gh", ["release", "edit", `v${version}`, "--repo", REPOSITORY, "--latest", "--title", `Buddy ${version}`, "--notes-file", notesFile]);
  }
  for (const relative of releaseAssets(files, version)) {
    const source = path.join(dir, relative);
    const filename = path.basename(source);
    const assets = JSON.parse(run("gh", ["release", "view", `v${version}`, "--repo", REPOSITORY, "--json", "assets"])).assets;
    if (assets.some((asset) => asset.name === filename)) {
      if (!sameGitHubAsset(version, filename, source)) throw new Error(`Refusing to replace GitHub Release asset: ${filename}`);
      console.log(`GitHub Release asset already verified: ${filename}`);
    } else {
      runVisible("gh", ["release", "upload", `v${version}`, source, "--repo", REPOSITORY]);
    }
  }
}

// Keeping this sequence independent from shell, filesystem, and network code
// makes the publication gate testable: stable.json has exactly one caller and
// is only reached after every immutable upload and Release attachment succeeds.
export async function publishSequence({ files, uploadArtifact, uploadNotes, uploadManifest, ensureRelease, assertVersion, switchStable }) {
  for (const relative of files) {
    await uploadArtifact(relative);
    await uploadArtifact(`${relative}.sig`);
  }
  await uploadNotes();
  await uploadManifest();
  await ensureRelease();
  await assertVersion();
  await switchStable();
}

async function probe() {
  const client = createOssClient();
  const runId = process.env.GITHUB_RUN_ID;
  const attempt = process.env.GITHUB_RUN_ATTEMPT;
  const probeId = runId && attempt && /^\d+$/.test(runId) && /^\d+$/.test(attempt)
    ? `${runId}-${attempt}`
    : randomUUID();
  const key = `buddy/releases/ci-check/${probeId}/connection.txt`;
  const probeFile = path.join(tmpdir(), `buddy-oss-probe-${probeId}.txt`);
  writeFileSync(probeFile, `Buddy OSS connectivity check ${new Date().toISOString()}\n`, "utf8");
  try {
    await putAndVerify(client, probeFile, key);
  } finally {
    rmSync(probeFile, { force: true });
  }
  console.log(`OSS connectivity probe passed: ${publicUrl(key)}`);
}

async function publish() {
  const version = requiredEnv("RELEASE_VERSION");
  versionParts(version);
  const commit = requiredEnv("RELEASE_COMMIT");
  requiredEnv("GH_TOKEN");
  assertSource(version, commit);
  await assertNewVersion(version);
  const dirIndex = process.argv.indexOf("--dir");
  if (dirIndex >= 0 && !process.argv[dirIndex + 1]) throw new Error("--dir requires a path");
  const dir = path.resolve(dirIndex >= 0 ? process.argv[dirIndex + 1] : path.join(".release", version));
  const files = requireArtifacts(dir, version);
  verifyArtifacts(dir, files);
  const notesFile = writeNotes(dir);
  const manifest = generateManifest(dir, version, notesFile);
  const client = createOssClient();
  await publishSequence({
    files,
    uploadArtifact: async (relative) => putAndVerify(client, path.join(dir, relative), `buddy/releases/${version}/${relative}`, { immutable: true, attempts: 2 }),
    uploadNotes: async () => putAndVerify(client, notesFile, `buddy/releases/${version}/notes.txt`, { contentType: "text/plain; charset=utf-8", attempts: 2 }),
    uploadManifest: async () => putAndVerify(client, manifest, `buddy/releases/${version}/manifest.json`, { contentType: "application/json; charset=utf-8", attempts: 2 }),
    // The GitHub mirror must be usable before updater clients see stable.json.
    ensureRelease: async () => ensureGitHubRelease(version, notesFile, dir, files),
    assertVersion: async () => assertNewVersion(version),
    // Do not retry the release switch. A failure is explicit and leaves its
    // outcome inspectable rather than making a second state-changing request.
    switchStable: async () => putAndVerify(client, manifest, "buddy/channels/stable.json", { contentType: "application/json; charset=utf-8" }),
  });
  console.log(`Published Buddy ${version}: ${publicUrl("buddy/channels/stable.json")}`);
}

async function main() {
  if (process.argv.slice(2).includes("--probe")) {
    if (process.argv.length !== 3) throw new Error("--probe cannot be combined with other arguments");
    await probe();
  } else {
    await publish();
  }
}

if (fileURLToPath(import.meta.url) === path.resolve(process.argv[1] ?? "")) {
  main().catch((error) => {
    console.error(`Release publishing failed: ${error.message}`);
    process.exitCode = 1;
  });
}
