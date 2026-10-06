import { test } from "node:test";
import assert from "node:assert/strict";
import { canonicalOssResource, compareVersions, fetchWithTimeout, ossAuthorization, ossStringToSign, publishSequence } from "./publish-ci-artifacts.mjs";

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
