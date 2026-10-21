// Assembles the per-platform npm packages (@hegeldev/hegel-<os>-<arch>) into
// `platform-packages/`. Each package contains a single libhegel shared library
// plus a manifest whose `os`/`cpu` fields let package managers install only the
// host's package. The main package lists them as exact-pinned
// `optionalDependencies`, but only in the published manifest: the pins are
// injected with `--pin` by the release pipeline just before `npm publish`
// (see .github/workflows/ci.yml), because a committed pin would name platform
// packages that do not exist yet, which `npm ci` rejects.
//
// Package versions are the main package's. The libraries are fetched from the
// libhegel release named by `libhegel/Cargo.toml` (see scripts/fetch-libhegel.mjs);
// `--local` instead packages the in-tree build for the host alone, which is what
// the consumer tests install.
//
// Usage:
//   node scripts/make-platform-packages.mjs            # every platform, from the release
//   node scripts/make-platform-packages.mjs --local    # the host, from the in-tree build
//   node scripts/make-platform-packages.mjs --pin      # ...and pin them in package.json

import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { ROOT, libhegelVersion, nativeLibraryPath } from "./engine.mjs";
import { PLATFORMS, fetchAsset, hostPlatform } from "./fetch-libhegel.mjs";

const OUT_DIR = path.join(ROOT, "platform-packages");
const MANIFEST = path.join(ROOT, "package.json");

const OS_LABELS = { darwin: "macOS", linux: "Linux", win32: "Windows" };
const CPU_LABELS = { x64: "64-bit", arm64: "ARM 64-bit" };

export function platformPackageName({ platform, arch }) {
  return `@hegeldev/hegel-${platform}-${arch}`;
}

// Package names use Node's platform/arch vocabulary (win32, x64) per ecosystem
// convention; the artifact filename keeps the release's Go-style tokens
// (windows, amd64), bridged by the "./binary" export.
function packageJson(target, { version, engines, repository, license }) {
  return {
    name: platformPackageName(target),
    version,
    description:
      `The ${OS_LABELS[target.platform]} ${CPU_LABELS[target.arch]} native library for ` +
      `@hegeldev/hegel, a property-based testing library for TypeScript.`,
    repository,
    license,
    // preferUnplugged is for yarn pnp, which uses zi pfiles by default, which fable
    // thinks won't work with koffi
    preferUnplugged: true,
    engines,
    os: [target.platform],
    cpu: [target.arch],
    exports: {
      "./binary": `./${target.asset}`,
    },
    publishConfig: {
      access: "public",
      provenance: true,
    },
  };
}

async function main() {
  const args = new Set(process.argv.slice(2));
  const local = args.delete("--local");
  const pin = args.delete("--pin");
  if (args.size > 0) {
    throw new Error(`unknown arguments: ${[...args].join(" ")}`);
  }
  const root = JSON.parse(fs.readFileSync(MANIFEST, "utf8"));
  const targets = local ? [hostPlatform()] : PLATFORMS;
  if (targets[0] === undefined) {
    throw new Error(`unsupported packaging host ${process.platform}/${process.arch}`);
  }

  fs.rmSync(OUT_DIR, { recursive: true, force: true });
  for (const target of targets) {
    const artifact = local
      ? nativeLibraryPath()
      : await fetchAsset(target.asset, libhegelVersion());
    const manifest = packageJson(target, root);
    const pkgDir = path.join(OUT_DIR, manifest.name.replace("@hegeldev/", ""));
    fs.mkdirSync(pkgDir, { recursive: true });
    fs.copyFileSync(artifact, path.join(pkgDir, target.asset));
    fs.writeFileSync(path.join(pkgDir, "package.json"), JSON.stringify(manifest, null, 2) + "\n");
  }
  if (pin) {
    root.optionalDependencies = Object.fromEntries(
      PLATFORMS.map((target) => [platformPackageName(target), root.version]).sort(),
    );
    fs.writeFileSync(MANIFEST, JSON.stringify(root, null, 2) + "\n");
  }
}

main().catch((err) => {
  process.stderr.write(`${err.message ?? err}\n`);
  process.exit(1);
});
