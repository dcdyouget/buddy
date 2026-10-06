import assert from "node:assert/strict";
import http from "node:http";
import { createHmac } from "node:crypto";
import { test } from "node:test";
import { canonicalQuery, multipartCanonicalResource, uploadMultipart } from "./oss-multipart.mjs";

async function serverFor(handler) {
  const server = http.createServer(handler);
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  const { port } = server.address();
  return {
    endpoint: `http://127.0.0.1:${port}`,
    close: () => new Promise((resolve, reject) => server.close((error) => error ? reject(error) : resolve())),
  };
}

function bodyOf(request) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    request.on("data", (chunk) => chunks.push(chunk));
    request.on("end", () => resolve(Buffer.concat(chunks)));
    request.on("error", reject);
  });
}

function options(endpoint, data) {
  return {
    bucket: "buddy-release", endpoint, accessKeyId: "test-access-id", accessKeySecret: "test-access-secret",
    key: "buddy/releases/0.2.0/windows/x86_64/Buddy.zip", data, timeoutMs: 5_000, ossHeaders: { "x-oss-meta-sha256": "test-digest" },
  };
}

test("multipart query is raw-sorted then RFC3986 encoded", () => {
  assert.equal(canonicalQuery([["uploadId", "a+/="], ["partNumber", 2], ["uploads", null]]), "partNumber=2&uploadId=a%2B%2F%3D&uploads");
  assert.equal(multipartCanonicalResource("buddy-release", "buddy/a b.zip", [["uploads", null]]), "/buddy-release/buddy/a%20b.zip?uploads");
});

test("multipart upload completes ordered parts over a local HTTP endpoint", async () => {
  const seen = [];
  const fixture = await serverFor(async (request, response) => {
    const url = new URL(request.url, "http://localhost");
    seen.push({ method: request.method, query: url.searchParams, body: await bodyOf(request), authorization: request.headers.authorization });
    assert.equal(request.headers["x-oss-meta-sha256"], "test-digest");
    const resource = multipartCanonicalResource("buddy-release", "buddy/releases/0.2.0/windows/x86_64/Buddy.zip", [...url.searchParams].map(([name, value]) => [name, name === "uploads" ? null : value]));
    const signature = createHmac("sha1", "test-access-secret").update(`${request.method}\n\n${request.headers["content-type"]}\n${request.headers.date}\nx-oss-meta-sha256:test-digest\n${resource}`).digest("base64");
    assert.equal(request.headers.authorization, `OSS test-access-id:${signature}`);
    if (request.method === "POST" && url.search === "?uploads") {
      response.end("<InitiateMultipartUploadResult><UploadId>upload%2Bid</UploadId></InitiateMultipartUploadResult>");
    } else if (request.method === "PUT") {
      response.setHeader("ETag", `\"part-${url.searchParams.get("partNumber")}\"`);
      response.end();
    } else if (request.method === "POST") {
      response.end("<CompleteMultipartUploadResult/>");
    } else {
      response.statusCode = 500; response.end();
    }
  });
  try {
    const data = Buffer.alloc(9 * 1024 * 1024, 7);
    assert.equal(await uploadMultipart(options(fixture.endpoint, data)), true);
    const parts = seen.filter((entry) => entry.method === "PUT");
    assert.equal(parts.length, 3);
    assert.deepEqual(parts.map((entry) => entry.query.get("partNumber")).sort(), ["1", "2", "3"]);
    assert.deepEqual(parts.map((entry) => entry.body.length).sort((a, b) => a - b), [1 * 1024 * 1024, 4 * 1024 * 1024, 4 * 1024 * 1024]);
    const complete = seen.find((entry) => entry.method === "POST" && entry.query.has("uploadId"));
    assert.match(complete.body.toString("utf8"), /<PartNumber>1<\/PartNumber>[\s\S]*<PartNumber>2<\/PartNumber>[\s\S]*<PartNumber>3<\/PartNumber>/);
  } finally {
    await fixture.close();
  }
});

test("a part failure aborts the multipart upload and never completes it", async () => {
  const seen = [];
  const fixture = await serverFor(async (request, response) => {
    const url = new URL(request.url, "http://localhost");
    seen.push({ method: request.method, query: url.search });
    await bodyOf(request);
    if (request.method === "POST" && url.search === "?uploads") {
      response.end("<InitiateMultipartUploadResult><UploadId>failed-upload</UploadId></InitiateMultipartUploadResult>");
    } else if (request.method === "PUT" && url.searchParams.get("partNumber") === "2") {
      response.statusCode = 500; response.end("failure");
    } else if (request.method === "PUT") {
      response.setHeader("ETag", "\"ok\""); response.end();
    } else if (request.method === "DELETE") {
      response.statusCode = 204; response.end();
    } else {
      response.statusCode = 500; response.end();
    }
  });
  try {
    await assert.rejects(uploadMultipart(options(fixture.endpoint, Buffer.alloc(9 * 1024 * 1024, 1))), /upload part failed: HTTP 500/);
    assert.ok(seen.some((entry) => entry.method === "DELETE" && entry.query.includes("uploadId=failed-upload")));
    assert.ok(!seen.some((entry) => entry.method === "POST" && entry.query.includes("uploadId=failed-upload")));
  } finally {
    await fixture.close();
  }
});
