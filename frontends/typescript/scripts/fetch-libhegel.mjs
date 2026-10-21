// Downloads the prebuilt libhegel shared libraries of a libhegel release into
// `native/<version>/`, each verified against the `<asset>.sha256` sidecar the
// release publishes next to it. The release pipeline uses this through
// `scripts/make-platform-packages.mjs` to assemble the per-platform npm
// packages for the libhegel version in `libhegel/Cargo.toml`, whose binaries
// hang off the monorepo's `libhegel/<version>` tag. Development and tests use
// the in-tree build instead (see scripts/engine.mjs).
//
// Usage:
//   node scripts/fetch-libhegel.mjs   # every platform's library; print the directory

import { createHash } from "node:crypto";
import { createWriteStream } from "node:fs";
import * as fs from "node:fs";
import { get as httpsGet } from "node:https";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { ROOT, libhegelVersion } from "./engine.mjs";

const NATIVE_DIR = path.join(ROOT, "native");

export const RELEASE_REPO = "hegeldev/hegel";

/** The tag that carries the libhegel release of a version. */
export function releaseTag(version) {
  return `libhegel/${version}`;
}

/** The download URL of an asset of the libhegel release of a version. */
export function assetUrl(asset, version) {
  return `https://github.com/${RELEASE_REPO}/releases/download/${releaseTag(version)}/${asset}`;
}

export const PLATFORMS = [
  { platform: "darwin", arch: "arm64", asset: "libhegel-darwin-arm64.dylib" },
  { platform: "linux", arch: "x64", asset: "libhegel-linux-amd64.so" },
  { platform: "linux", arch: "arm64", asset: "libhegel-linux-arm64.so" },
  { platform: "win32", arch: "x64", asset: "libhegel-windows-amd64.dll" },
  { platform: "win32", arch: "arm64", asset: "libhegel-windows-arm64.dll" },
];

/** The PLATFORMS entry for the host, or undefined on an unsupported host. */
export function hostPlatform() {
  return PLATFORMS.find((p) => p.platform === process.platform && p.arch === process.arch);
}

export function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

/**
 * The hex digest in a release's `<asset>.sha256` sidecar: `sha256sum` output
 * (`<hex>  <asset>`, or `<hex> *<asset>` in binary mode) or a bare digest.
 */
export function sidecarDigest(text, asset) {
  const match = /^([a-f0-9]{64})(?:\s+\*?(\S+))?$/.exec(text.trim());
  if (!match || (match[2] !== undefined && match[2] !== asset)) {
    throw new Error(`libhegel: malformed checksum sidecar for ${asset}`);
  }
  return match[1];
}

/** Stream a URL to a file, following redirects. */
function download(url, dest, redirects = 5) {
  return new Promise((resolve, reject) => {
    httpsGet(url, (res) => {
      const status = res.statusCode ?? 0;
      if (status >= 300 && status < 400 && res.headers.location) {
        res.resume();
        if (redirects <= 0) {
          reject(new Error("too many redirects"));
          return;
        }
        download(new URL(res.headers.location, url).toString(), dest, redirects - 1).then(
          resolve,
          reject,
        );
        return;
      }
      if (status !== 200) {
        res.resume();
        reject(new Error(`HTTP ${status} for ${url}`));
        return;
      }
      const out = createWriteStream(dest);
      out.on("error", reject);
      out.on("finish", () => resolve());
      res.pipe(out);
    }).on("error", reject);
  });
}

/**
 * Fetch a single asset of the given release into native/<version>/, verifying
 * it against the checksum sidecar published next to it.
 */
export async function fetchAsset(asset, version) {
  const dest = path.join(NATIVE_DIR, version, asset);
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  if (fs.existsSync(dest)) {
    process.stderr.write(`libhegel: ${asset} already present\n`);
    return dest;
  }
  const url = assetUrl(asset, version);
  const tmp = `${dest}.${process.pid}.partial`;
  const sidecar = `${tmp}.sha256`;
  process.stderr.write(`libhegel: downloading ${asset}\n`);
  try {
    await download(url, tmp);
    await download(`${url}.sha256`, sidecar);
    const expected = sidecarDigest(fs.readFileSync(sidecar, "utf8"), asset);
    if (sha256(fs.readFileSync(tmp)) !== expected) {
      throw new Error(`libhegel: ${asset} does not match its published checksum`);
    }
    fs.chmodSync(tmp, 0o755);
    fs.renameSync(tmp, dest);
  } finally {
    fs.rmSync(tmp, { force: true });
    fs.rmSync(sidecar, { force: true });
  }
  return dest;
}

async function main(args) {
  if (args.length > 0) {
    throw new Error("usage: fetch-libhegel.mjs");
  }
  const version = libhegelVersion();
  for (const { asset } of PLATFORMS) {
    await fetchAsset(asset, version);
  }
  process.stdout.write(`${path.join(NATIVE_DIR, version)}\n`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch((err) => {
    process.stderr.write(`${err.message ?? err}\n`);
    process.exit(1);
  });
}
