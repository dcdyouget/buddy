// Minimal Aliyun OSS V1 multipart uploader. It intentionally uses node:http
// and node:https rather than undici so each small part has an independent
// socket and header timeout.
import { createHmac } from "node:crypto";
import http from "node:http";
import https from "node:https";

const PART_SIZE = 4 * 1024 * 1024;
const CONCURRENCY = 4;

function rfc3986(value) {
  return encodeURIComponent(value).replace(/[!'()*]/g, (character) => `%${character.charCodeAt(0).toString(16).toUpperCase()}`);
}

function validateKey(key) {
  if (typeof key !== "string" || !key || key.startsWith("/") || key.split("/").some((part) => !part || part === "." || part === "..")) {
    throw new Error("Invalid OSS object key");
  }
}

function compareRaw(left, right) {
  return left < right ? -1 : left > right ? 1 : 0;
}

// Query parameter names and values are sorted before encoding. The same
// representation is used for the wire request and the V1 canonical resource.
export function canonicalQuery(entries) {
  return [...entries]
    .map(([name, value]) => [String(name), value === null || value === undefined ? null : String(value)])
    .sort(([leftName, leftValue], [rightName, rightValue]) => compareRaw(leftName, rightName) || compareRaw(leftValue ?? "", rightValue ?? ""))
    .map(([name, value]) => value === null ? rfc3986(name) : `${rfc3986(name)}=${rfc3986(value)}`)
    .join("&");
}

export function multipartCanonicalResource(bucket, key, query) {
  validateKey(key);
  if (!bucket) throw new Error("OSS bucket is required");
  const object = key.split("/").map(rfc3986).join("/");
  const encodedQuery = canonicalQuery(query);
  return `/${bucket}/${object}${encodedQuery ? `?${encodedQuery}` : ""}`;
}

function stringToSign(method, contentType, date, resource) {
  return `${method}\n\n${contentType}\n${date}\n${resource}`;
}

function authorization(accessKeyId, accessKeySecret, method, contentType, date, resource) {
  const signature = createHmac("sha1", accessKeySecret).update(stringToSign(method, contentType, date, resource)).digest("base64");
  return `OSS ${accessKeyId}:${signature}`;
}

function request(url, { method, headers, body, timeoutMs }) {
  const transport = url.protocol === "https:" ? https : url.protocol === "http:" ? http : null;
  if (!transport) throw new Error(`Unsupported OSS endpoint protocol: ${url.protocol}`);
  return new Promise((resolve, reject) => {
    const requestOptions = {
      protocol: url.protocol,
      hostname: url.hostname,
      port: url.port || undefined,
      method,
      path: `${url.pathname}${url.search}`,
      headers: { ...headers, "Content-Length": String(body.length) },
    };
    let settled = false;
    const finish = (callback, value) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      callback(value);
    };
    const req = transport.request(requestOptions, (response) => {
      const chunks = [];
      response.on("data", (chunk) => chunks.push(chunk));
      response.on("error", (error) => finish(reject, error));
      response.on("end", () => finish(resolve, {
        statusCode: response.statusCode ?? 0,
        headers: response.headers,
        body: Buffer.concat(chunks),
      }));
    });
    const timer = setTimeout(() => req.destroy(new Error(`OSS request timed out after ${timeoutMs}ms`)), timeoutMs);
    req.on("error", (error) => finish(reject, error));
    req.end(body);
  });
}

function xmlEscape(value) {
  return String(value).replace(/[&<>]/g, (character) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[character]);
}

function xmlUnescape(value) {
  return value.replace(/&(?:amp|lt|gt|quot|apos);/g, (entity) => ({ "&amp;": "&", "&lt;": "<", "&gt;": ">", "&quot;": '"', "&apos;": "'" })[entity]);
}

function responseError(stage, response) {
  const code = /<Code>([A-Za-z0-9_-]{1,80})<\/Code>/.exec(response.body.toString("utf8"))?.[1] ?? "Unknown";
  return new Error(`OSS multipart ${stage} failed: HTTP ${response.statusCode}, code ${code}`);
}

function getHeader(headers, name) {
  const value = headers[name.toLowerCase()];
  return Array.isArray(value) ? value[0] : value;
}

function partsFor(data) {
  if (!Buffer.isBuffer(data) || data.length === 0) throw new Error("Multipart data must be a non-empty Buffer");
  const parts = [];
  for (let offset = 0, number = 1; offset < data.length; offset += PART_SIZE, number += 1) {
    parts.push({ number, data: data.subarray(offset, Math.min(offset + PART_SIZE, data.length)) });
  }
  if (parts.length > 10_000) throw new Error("OSS multipart upload exceeds 10,000 parts");
  return parts;
}

function endpointUrl(endpoint, key, query) {
  const base = new URL(endpoint);
  if (base.protocol !== "https:" && base.protocol !== "http:") throw new Error("OSS endpoint must use HTTP(S)");
  if (base.username || base.password || (base.pathname !== "/" && base.pathname !== "")) throw new Error("OSS endpoint must not contain credentials or a path");
  const object = key.split("/").map(rfc3986).join("/");
  const encodedQuery = canonicalQuery(query);
  return new URL(`/${object}${encodedQuery ? `?${encodedQuery}` : ""}`, base);
}

function createSignedRequester({ bucket, endpoint, accessKeyId, accessKeySecret, key, timeoutMs }) {
  if (!accessKeyId || !accessKeySecret) throw new Error("OSS access key ID and secret are required");
  validateKey(key);
  return async (stage, method, query, body, contentType) => {
    const date = new Date().toUTCString();
    const resource = multipartCanonicalResource(bucket, key, query);
    const response = await request(endpointUrl(endpoint, key, query), {
      method,
      timeoutMs,
      body,
      headers: {
        Authorization: authorization(accessKeyId, accessKeySecret, method, contentType, date, resource),
        Date: date,
        "Content-Type": contentType,
        "Cache-Control": "no-cache",
      },
    });
    if (response.statusCode < 200 || response.statusCode >= 300 || /<Error[>\s]/.test(response.body.toString("utf8"))) throw responseError(stage, response);
    return response;
  };
}

async function uploadParts(parts, uploadPart) {
  const completed = new Array(parts.length);
  let next = 0;
  let failure;
  const worker = async () => {
    while (!failure) {
      const index = next;
      next += 1;
      if (index >= parts.length) return;
      try {
        completed[index] = await uploadPart(parts[index]);
      } catch (error) {
        failure = error;
      }
    }
  };
  await Promise.all(Array.from({ length: Math.min(CONCURRENCY, parts.length) }, worker));
  if (failure) throw failure;
  return completed;
}

/**
 * Upload a Buffer as an OSS multipart object.
 * @returns {Promise<true>}
 */
export async function uploadMultipart({ bucket, endpoint, accessKeyId, accessKeySecret, key, data, contentType = "application/octet-stream", timeoutMs = 600_000 }) {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs <= 0) throw new Error("timeoutMs must be a positive integer");
  const parts = partsFor(data);
  const signedRequest = createSignedRequester({ bucket, endpoint, accessKeyId, accessKeySecret, key, timeoutMs });
  const initiated = await signedRequest("initiate", "POST", [["uploads", null]], Buffer.alloc(0), contentType);
  const uploadIdText = /<UploadId>([\s\S]*?)<\/UploadId>/.exec(initiated.body.toString("utf8"))?.[1];
  const uploadId = uploadIdText && xmlUnescape(uploadIdText);
  if (!uploadId) throw new Error("OSS multipart initiate response did not contain UploadId");
  try {
    const completed = await uploadParts(parts, async (part) => {
      const response = await signedRequest("upload part", "PUT", [["partNumber", part.number], ["uploadId", uploadId]], part.data, contentType);
      const etag = getHeader(response.headers, "etag");
      if (!etag) throw new Error(`OSS multipart part ${part.number} did not return ETag`);
      return { number: part.number, etag };
    });
    const completeXml = `<?xml version="1.0" encoding="UTF-8"?><CompleteMultipartUpload>${completed
      .sort((left, right) => left.number - right.number)
      .map((part) => `<Part><PartNumber>${part.number}</PartNumber><ETag>${xmlEscape(part.etag)}</ETag></Part>`).join("")}</CompleteMultipartUpload>`;
    await signedRequest("complete", "POST", [["uploadId", uploadId]], Buffer.from(completeXml, "utf8"), "application/xml");
    return true;
  } catch (error) {
    try {
      await signedRequest("abort", "DELETE", [["uploadId", uploadId]], Buffer.alloc(0), "application/xml");
    } catch {
      // The original upload failure remains the actionable error. Abort is
      // best effort and does not expose response contents.
    }
    throw error;
  }
}
