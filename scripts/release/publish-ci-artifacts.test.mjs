import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { createHash } from "node:crypto";
import { canonicalOssResource, compareVersions, fetchWithTimeout, ossAuthorization, ossStringToSign, publishSequence, releaseAssets, headPublic, putAndVerify } from "./publish-ci-artifacts.mjs";

test("GitHub Release lists installers while OSS retains update and portable artifacts", () => {
  const files = ["macos/aarch64/Buddy_0.1.16_aarch64.app.tar.xz", "macos/aarch64/Buddy_0.1.16_aarch64.dmg", "windows/x86_64/Buddy_0.1.16_x86_64.exe", "windows/x86_64/Buddy_0.1.16_x86_64_setup.exe", "windows/x86_64/Buddy_0.1.16_x86_64.zip"];
  assert.deepEqual(releaseAssets(files, "0.1.16"), [files[1], files[3]]);
  assert.deepEqual(releaseAssets(files.filter((file) => !file.endsWith("_setup.exe")), "0.1.16"), [files[1]]);
});

test("OSS V1 canonical resource rejects unsafe object keys", () => {
  assert.equal(canonicalOssResource("buddy-release", "buddy/releases/0.2.0/manifest.json"), "/buddy-release/buddy/releases/0.2.0/manifest.json");
  assert.throws(() => canonicalOssResource("buddy-release", "buddy/releases/../stable.json"));
});

test("OSS V1 signing canonicalizes x-oss headers", () => {
  const input = {
    method: "PUT", contentType: "application/json", date: "Tue, 07 Oct 2026 00:00:00 GMT",
    bucket: "buddy-release", key: "buddy/channels/stable.json",
    ossHeaders: { "X-OSS-Meta-Release": " 0.2.0 ", "x-oss-acl": "private" },
  };
  assert.equal(ossStringToSign(input), "PUT\n\napplication/json\nTue, 07 Oct 2026 00:00:00 GMT\nx-oss-acl:private\nx-oss-meta-release:0.2.0\n/buddy-release/buddy/channels/stable.json");
  assert.match(ossAuthorization({ ...input, accessKeyId: "test-id", accessKeySecret: "test-secret" }), /^OSS test-id:[A-Za-z0-9+/]+=*$/);
});

test("versions compare numerically", () => {
  assert.equal(compareVersions("0.1.10", "0.1.9"), 1);
  assert.equal(compareVersions("1.0.0", "1.0.0"), 0);
  assert.equal(compareVersions("1.0.0", "1.0.1"), -1);
  assert.throws(() => compareVersions("v1.0.0", "1.0.0"));
});

function sequenceCallbacks(events, failAt) {
  const step = (name) => async () => {
    events.push(name);
    if (name === failAt) throw new Error(`${name} failed`);
  };
  return {
    uploadArtifact: async (name) => step(`artifact:${name}`)(),
    uploadNotes: step("notes"),
    uploadManifest: step("manifest"),
    ensureRelease: step("release"),
    assertVersion: step("version"),
    switchStable: step("stable"),
  };
}

test("an artifact upload failure never switches stable.json", async () => {
  const events = [];
  await assert.rejects(publishSequence({ files: ["a"], ...sequenceCallbacks(events, "artifact:a.sig") }), /artifact:a.sig failed/);
  assert.deepEqual(events, ["artifact:a", "artifact:a.sig"]);
  assert.ok(!events.includes("stable"));
});

test("a GitHub Release attachment failure never switches stable.json", async () => {
  const events = [];
  await assert.rejects(publishSequence({ files: ["a"], ...sequenceCallbacks(events, "release") }), /release failed/);
  assert.deepEqual(events, ["artifact:a", "artifact:a.sig", "notes", "manifest", "release"]);
  assert.ok(!events.includes("stable"));
});

test("stable.json is the last publication operation", async () => {
  const events = [];
  await publishSequence({ files: ["a"], ...sequenceCallbacks(events) });
  assert.deepEqual(events, ["artifact:a", "artifact:a.sig", "notes", "manifest", "release", "version", "stable"]);
});

async function withMockFetch(mock, run) {
  const originalFetch = globalThis.fetch;
  globalThis.fetch = mock;
  try {
    await run();
  } finally {
    globalThis.fetch = originalFetch;
  }
}

test("a transient public GET connection failure retries and succeeds", { concurrency: false }, async () => {
  let calls = 0;
  await withMockFetch(async () => {
    calls += 1;
    if (calls === 1) {
      const error = new TypeError("fetch failed");
      error.cause = { code: "UND_ERR_SOCKET" };
      throw error;
    }
    return new Response("ok", { status: 200 });
  }, async () => {
    const response = await fetchWithTimeout("https://example.invalid/public", {}, { attempts: 3 });
    assert.equal(await response.text(), "ok");
  });
  assert.equal(calls, 2);
});

test("a transient 5xx response retries and succeeds", { concurrency: false }, async () => {
  let calls = 0;
  await withMockFetch(async () => {
    calls += 1;
    return calls === 1 ? new Response("busy", { status: 503 }) : new Response("ok", { status: 200 });
  }, async () => {
    const response = await fetchWithTimeout("https://example.invalid/public", {}, { attempts: 2 });
    assert.equal(response.status, 200);
  });
  assert.equal(calls, 2);
});

test("retry limit and stable PUT default both make no extra request", { concurrency: false }, async () => {
  let boundedCalls = 0;
  await withMockFetch(async () => {
    boundedCalls += 1;
    throw new TypeError("fetch failed");
  }, async () => {
    await assert.rejects(fetchWithTimeout("https://example.invalid/public", {}, { attempts: 2 }), /Network request failed/);
  });
  assert.equal(boundedCalls, 2);

  let stablePutCalls = 0;
  await withMockFetch(async () => {
    stablePutCalls += 1;
    throw new TypeError("fetch failed");
  }, async () => {
    await assert.rejects(fetchWithTimeout("https://example.invalid/stable.json", { method: "PUT", body: "manifest" }), /Network request failed/);
  });
  assert.equal(stablePutCalls, 1);
});


test("publication verifies HEAD only and refuses conflicting immutable files", { concurrency: false }, async () => {
  const dir = mkdtempSync(path.join(tmpdir(), "buddy-head-test-"));
  const file = path.join(dir, "artifact");
  const data = Buffer.from("signed artifact");
  writeFileSync(file, data);
  const digest = createHash("sha256").update(data).digest("hex");
  const key = "buddy/releases/test/artifact";
  let heads = 0;
  let puts = 0;
  const client = { request: async (method, object, body, type, attempts, metadata) => {
    puts += 1;
    assert.equal(method, "PUT");
    assert.equal(object, key);
    assert.deepEqual(body, data);
    assert.equal(metadata["x-oss-meta-sha256"], digest);
  } };
  try {
    await withMockFetch(async (url, init) => {
      assert.equal(init.method, "HEAD");
      heads += 1;
      if (heads === 1) return new Response(null, { status: 404 });
      return new Response(null, { headers: { "Content-Length": String(data.length), "x-oss-meta-sha256": digest } });
    }, async () => {
      await putAndVerify(client, file, key, { immutable: true });
      await putAndVerify(client, file, key, { immutable: true });
    });
    assert.equal(puts, 1);
    await withMockFetch(async () => new Response(null, { headers: { "Content-Length": String(data.length), "x-oss-meta-sha256": "different" } }), async () => {
      await assert.rejects(putAndVerify(client, file, key, { immutable: true }), /Refusing to replace/);
    });
    assert.equal(puts, 1);
    await withMockFetch(async () => new Response(null, { headers: { "Content-Length": "1" } }), async () => {
      await assert.rejects(putAndVerify(client, file, key), /HEAD size mismatch/);
    });
    await withMockFetch(async () => new Response(null), async () => {
      await assert.rejects(headPublic(key), /Content-Length/);
    });
    await withMockFetch(async () => new Response(null, { status: 403 }), async () => {
      await assert.rejects(headPublic(key), /HTTP 403/);
    });
  } finally { rmSync(dir, { recursive: true, force: true }); }
});
