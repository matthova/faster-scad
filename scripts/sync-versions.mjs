#!/usr/bin/env node
// Propagate the canonical version from the root package.json (just bumped by
// `changeset version`) into every other version-bearing file in the repo.
//
// This is the changesets replacement for release-please's `extra-files`: the
// repo keeps ONE version across Rust, Tauri, and the npm package, but changesets
// only understands package.json, so this script owns the rest. It is invoked by
// `npm run version` (see package.json) immediately after `changeset version`.
//
// Deliberately NOT synced (kept off the shared version, matching prior policy):
//   - web/package.json           (private playground, never published)
//   - editors/vscode/package.json (ships to the VS Code marketplace on its own cadence)
//   - fuzz/Cargo.toml            (publish = false)
//
// Pure text/JSON edits only — no cargo/npm toolchain required, so the CI version
// job stays fast and offline. Edits are surgical to keep diffs minimal.
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

const rootUrl = new URL("../", import.meta.url); // repo root (this file is in scripts/)
const abs = (rel) => fileURLToPath(new URL(rel, rootUrl));
const read = (rel) => readFileSync(abs(rel), "utf8");
const write = (rel, s) => writeFileSync(abs(rel), s);

const version = JSON.parse(read("package.json")).version;
if (!/^\d+\.\d+\.\d+([-+].+)?$/.test(version)) {
  throw new Error(`root package.json version '${version}' is not a semver`);
}

const changed = [];

// TOML: the [package]/[workspace.package] version is the first `version = "..."`
// line in these files (it precedes every dependency table). Replace just it.
// Guards on the pattern being PRESENT (not on the content changing), so a re-run
// at the current version is a clean no-op rather than a false "not found" error.
function bumpTomlPackageVersion(rel) {
  const src = read(rel);
  const re = /^version = "[^"]*"/m;
  if (!re.test(src)) throw new Error(`no [package] version line found in ${rel}`);
  write(rel, src.replace(re, `version = "${version}"`));
  changed.push(rel);
}

// Cargo.lock: bump the entries for our own crates (quito-* and xtask). Dependency
// references live under `dependencies = [...]` as quoted strings, so anchoring
// `name = "..."` to the start of a line only matches [[package]] definitions.
function bumpCargoLock(rel) {
  const src = read(rel);
  const re = /(^name = "(?:quito-[a-z]+|xtask)"\nversion = ")[^"]*(")/gm;
  if (!re.test(src)) throw new Error(`no quito-*/xtask entries found in ${rel}`);
  write(rel, src.replace(re, `$1${version}$2`));
  changed.push(rel);
}

// JSON: set specific paths and re-emit as 2-space JSON (npm's format), so only
// the touched version fields change.
function bumpJson(rel, setter) {
  const obj = JSON.parse(read(rel));
  setter(obj, version);
  write(rel, JSON.stringify(obj, null, 2) + "\n");
  changed.push(rel);
}

// --- Rust workspace (engine, CLI, wasm, xtask, ...) ---
bumpTomlPackageVersion("Cargo.toml");
bumpCargoLock("Cargo.lock");

// --- Desktop (standalone Tauri workspace) ---
bumpTomlPackageVersion("desktop/src-tauri/Cargo.toml");
bumpCargoLock("desktop/src-tauri/Cargo.lock");
bumpJson("desktop/src-tauri/tauri.conf.json", (o, v) => (o.version = v));
bumpJson("desktop/package.json", (o, v) => (o.version = v));
bumpJson("desktop/package-lock.json", (o, v) => {
  o.version = v;
  if (o.packages && o.packages[""]) o.packages[""].version = v;
});

// --- Published npm package (quito-engine) ---
bumpJson("packages/npm/package.json", (o, v) => (o.version = v));
bumpJson("packages/npm/package-lock.json", (o, v) => {
  o.version = v;
  if (o.packages && o.packages[""]) o.packages[""].version = v;
});

// --- Root tooling lockfile (private; keeps `npm ci` happy) ---
if (existsSync(abs("package-lock.json"))) {
  bumpJson("package-lock.json", (o, v) => {
    o.version = v;
    if (o.packages && o.packages[""]) o.packages[""].version = v;
  });
}

console.log(`synced version ${version} into:`);
for (const f of changed) console.log(`  ${f}`);
