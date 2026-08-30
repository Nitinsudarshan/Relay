#!/usr/bin/env node
/**
 * Provisions Relay's voice catalogue with real, pinned checksums.
 *
 * `native/src-tauri/resources/voice-manifest.json` is checked in with its
 * `sha256` and `size_bytes` fields empty, because a checksum cannot be
 * written by hand — an invented one fails every install, and a copied one
 * nobody verified is worse than none at all. Until this script has run,
 * `VoiceManifest::validate()` rejects the file and Relay reports that
 * automatic voice setup is unavailable rather than downloading something
 * it cannot check.
 *
 * This is a **release step**, run once per manifest change on a machine
 * with network access:
 *
 *     node scripts/build-voice-manifest.mjs
 *     node scripts/build-voice-manifest.mjs --check       # verify, don't write
 *     node scripts/build-voice-manifest.mjs --tag 2023.11.14-2
 *
 * ## Why it resolves assets by name and not by pattern
 *
 * It used to match the release's asset list against a regular expression —
 * "windows, x86_64, ends in .zip". That is a guess about a project's naming
 * habits, and it broke the moment the project it was pointed at stopped
 * publishing an executable: `OHF-Voice/piper1-gpl` ships Python wheels, and
 * the matcher had nothing to match, so the run failed with "no asset for
 * piper-windows-x86_64" and no explanation of why.
 *
 * A looser pattern would have been worse. `piper_tts-1.7.0-cp39-abi3-
 * win_amd64.whl` *is* a zip, and one relaxed regex away from being
 * downloaded, hashed, pinned, and shipped to users as a speech engine it
 * cannot possibly be.
 *
 * So each runtime in the manifest names its `release` — repo, tag, asset —
 * and this script resolves that exact name and nothing else. Then it opens
 * the archive and checks the executable the manifest promises is really
 * inside it. The manifest, the artifact and this script cannot disagree
 * without the run failing.
 */

import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { ArchiveError, indexArchive, requireExecutable } from './lib/archive-index.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const MANIFEST = path.join(ROOT, 'native/src-tauri/resources/voice-manifest.json');

/** The schema this script knows how to fill in. Matches `manifest.rs`. */
const SCHEMA_VERSION = 2;

const args = process.argv.slice(2);
const checkOnly = args.includes('--check');
const tagOverride = valueOf('--tag') ?? valueOf('--piper-tag');

/** Set while a progress line is open, so a failure never eats it. */
let lineIsOpen = false;

/** A problem worth stopping for, phrased for whoever is running this. */
class FatalError extends Error {}

function valueOf(flag) {
  const index = args.indexOf(flag);
  return index >= 0 ? args[index + 1] : undefined;
}

function fail(message) {
  throw new FatalError(message);
}

function beginLine(text) {
  process.stdout.write(text);
  lineIsOpen = true;
}

function endLine(text) {
  process.stdout.write(`${text}\n`);
  lineIsOpen = false;
}

/**
 * Downloads a URL fully, returning its bytes, sha256 and size.
 *
 * The body is always drained or cancelled. A response left half-read holds
 * its socket open, and a process that exits with one of those in flight is
 * how this script used to abort on Windows with a libuv assertion instead
 * of the error it was trying to report.
 */
async function fetchAndHash(url, label) {
  beginLine(`  ${label} … `);

  let response;
  try {
    response = await fetch(url, { redirect: 'follow' });
  } catch (error) {
    fail(`${label}: could not reach ${url}\n  ${error.message}`);
  }

  if (!response.ok) {
    await response.body?.cancel().catch(() => {});
    fail(`${label}: ${response.status} ${response.statusText}\n  ${url}`);
  }

  const bytes = Buffer.from(await response.arrayBuffer());
  if (bytes.length === 0) fail(`${label}: empty response from ${url}`);

  const sha256 = createHash('sha256').update(bytes).digest('hex');
  endLine(`${(bytes.length / 1e6).toFixed(1)} MB  ${sha256.slice(0, 12)}…`);
  return { bytes, sha256, size_bytes: bytes.length };
}

/**
 * Where release *metadata* is read from.
 *
 * Overridable so the end-to-end path can be exercised from a network that
 * cannot reach the GitHub API — and only that path. It has no bearing on
 * what gets pinned: the download URL is derived from the manifest's own
 * release pin, the checksum is computed from the bytes that arrive, and
 * the archive is opened and checked. A wrong asset list can only make the
 * run fail, never make it pin something. The override announces itself
 * loudly for exactly that reason.
 */
const GITHUB_API = process.env.RELAY_GITHUB_API || 'https://api.github.com';

/** Reads one release from the GitHub API. */
async function fetchRelease(repo, tag) {
  const endpoint = `${GITHUB_API}/repos/${repo}/releases/tags/${encodeURIComponent(tag)}`;

  let response;
  try {
    response = await fetch(endpoint, {
      headers: {
        Accept: 'application/vnd.github+json',
        'User-Agent': 'relay-voice-manifest',
        // Optional, but avoids the unauthenticated rate limit in CI.
        ...(process.env.GITHUB_TOKEN
          ? { Authorization: `Bearer ${process.env.GITHUB_TOKEN}` }
          : {}),
      },
    });
  } catch (error) {
    fail(`Could not reach the GitHub API for ${repo}\n  ${error.message}`);
  }

  if (!response.ok) {
    await response.body?.cancel().catch(() => {});
    if (response.status === 404) {
      fail(`${repo} has no release tagged ${tag}.`);
    }
    fail(
      `Could not read ${repo}@${tag} (${response.status} ${response.statusText}). ` +
        'Set GITHUB_TOKEN if you are being rate-limited.',
    );
  }
  return response.json();
}

/**
 * Finds the one asset the manifest names, or explains what is there instead.
 *
 * The diagnostic matters as much as the lookup. A release that publishes
 * only Python wheels is not a release with a misnamed asset, and telling
 * someone "no asset for piper-windows-x86_64" invites them to go looking
 * for one to rename it to.
 */
function findAsset(release, runtime) {
  const wanted = runtime.release.asset;
  const assets = release.assets ?? [];
  const asset = assets.find((candidate) => candidate.name === wanted);
  if (asset) return asset;

  const names = assets.map((a) => a.name);
  const lines = [
    `${release.tag_name} has no asset called ${wanted} (needed for ${runtime.id}).`,
    names.length ? `  It publishes:\n${names.map((n) => `    ${n}`).join('\n')}` : '  It publishes nothing.',
  ];

  const wheels = names.filter((name) => name.endsWith('.whl'));
  if (wheels.length > 0 && wheels.length + names.filter((n) => n.endsWith('.tar.gz')).length === names.length) {
    lines.push(
      '',
      '  Every asset here is a Python distribution. A wheel is a package for',
      '  an interpreter, not a program: it needs CPython and onnxruntime',
      '  installed before anything in it can run, and it contains no',
      `  ${path.posix.basename(runtime.executable_path)}.`,
      '',
      '  Relay installs a standalone executable and spawns it, so it cannot',
      '  use this release. Point `release.repo`/`release.tag` in the manifest',
      '  at a project that publishes one — or teach Relay a second install',
      '  strategy first. Do not rename the expected asset to whatever is',
      '  here: the download would succeed and the engine would never start.',
    );
  }
  fail(lines.join('\n'));
}

/**
 * Refuses a manifest this script should not be filling in.
 *
 * Mirrors the checks in `manifest.rs`, so a catalogue that Relay would
 * reject at startup never reaches a commit with real checksums attached
 * to make it look trustworthy.
 */
function checkManifestShape(manifest) {
  const problems = [];

  for (const runtime of manifest.runtimes ?? []) {
    const label = `runtime ${runtime.id}`;
    const release = runtime.release;

    if (!release?.repo || !release.tag || !release.asset) {
      problems.push(`${label}: no release {repo, tag, asset}`);
      continue;
    }
    if (!/^[^/]+\/[^/]+$/.test(release.repo)) {
      problems.push(`${label}: release repo must be owner/name`);
    }
    if (release.asset.includes('/') || release.tag.includes('/')) {
      problems.push(`${label}: release tag and asset must be plain names`);
    }
    if (release.asset.toLowerCase().endsWith('.whl')) {
      problems.push(`${label}: ${release.asset} is a Python wheel, not a standalone engine`);
    }
    if (runtime.source !== `https://github.com/${release.repo}`) {
      problems.push(`${label}: source is not the project the artifact comes from`);
    }

    const impliedKind = archiveKindOf(release.asset);
    if (runtime.archive !== 'raw' && impliedKind !== runtime.archive) {
      problems.push(
        `${label}: declared "${runtime.archive}" but ${release.asset} is ` +
          `${impliedKind ?? 'not an archive Relay can unpack'}`,
      );
    }
    if (runtime.archive !== 'raw') {
      const exe = runtime.executable_path ?? '';
      if (!exe) problems.push(`${label}: archive with no executable path`);
      else if (exe.includes('\\') || exe.startsWith('/') || exe.includes(':') || exe.split('/').includes('..')) {
        problems.push(`${label}: executable path must be relative and forward-slashed`);
      }
    }
  }

  const recommended = (manifest.voices ?? []).filter((v) => v.recommended);
  if (recommended.length !== 1) {
    problems.push(`exactly one voice must be recommended; found ${recommended.length}`);
  }

  if (problems.length > 0) {
    fail(`The manifest is not one this script will provision:\n  ${problems.join('\n  ')}`);
  }
}

function archiveKindOf(name) {
  const lower = name.toLowerCase();
  if (lower.endsWith('.tar.gz') || lower.endsWith('.tgz')) return 'tar_gz';
  if (lower.endsWith('.zip')) return 'zip';
  return null;
}

/** Canonical GitHub download URL for a pinned release asset. */
function downloadUrl({ repo, tag, asset }) {
  return `https://github.com/${repo}/releases/download/${tag}/${asset}`;
}

/**
 * The claim the manifest makes about a runtime, checked against the bytes.
 *
 * Everything up to here is names: an asset called this, in a release called
 * that. This is the only step that looks inside.
 */
function verifyRuntimeArchive(runtime, bytes) {
  if (runtime.archive === 'raw') return;

  let entries;
  try {
    entries = indexArchive(runtime.archive, bytes);
  } catch (error) {
    if (error instanceof ArchiveError) {
      fail(`${runtime.id}: ${runtime.release.asset} could not be read as ${runtime.archive}.\n  ${error.message}`);
    }
    throw error;
  }

  try {
    const entry = requireExecutable(entries, runtime.executable_path, {
      // A zip made on Windows carries no Unix modes, so the bit is only
      // required where it exists to be required.
      requireExecutableBit: runtime.archive === 'tar_gz',
    });
    console.log(
      `    contains ${entry.path} (${(entry.size / 1e6).toFixed(1)} MB) ` +
        `among ${entries.size} entries`,
    );
  } catch (error) {
    if (error instanceof ArchiveError) {
      fail(
        `${runtime.id}: ${runtime.release.asset} is not an engine Relay can run.\n  ${error.message}`,
      );
    }
    throw error;
  }
}

async function provisionRuntimes(manifest) {
  console.log('\nResolving voice engines…');

  // One API call per distinct release, not per runtime: Windows and Linux
  // come from the same tag.
  const releases = new Map();

  for (const runtime of manifest.runtimes) {
    if (tagOverride) runtime.release.tag = tagOverride;
    const { repo, tag } = runtime.release;
    const key = `${repo}@${tag}`;

    if (!releases.has(key)) {
      releases.set(key, await fetchRelease(repo, tag));
      console.log(`  ${key} — ${(releases.get(key).assets ?? []).length} assets`);
    }
    const release = releases.get(key);
    const asset = findAsset(release, runtime);

    // The URL is derived from the pinned release, not taken from the API,
    // and then required to agree with what the API returned. Relay
    // re-derives the same URL at load time; if these three ever disagree,
    // that is the interesting event, not something to paper over.
    const url = downloadUrl(runtime.release);
    if (asset.browser_download_url !== url) {
      fail(
        `${runtime.id}: GitHub serves ${runtime.release.asset} from\n` +
          `    ${asset.browser_download_url}\n` +
          `  but the manifest's release pin derives\n    ${url}`,
      );
    }

    const hashed = await fetchAndHash(url, runtime.id);
    verifyRuntimeArchive(runtime, hashed.bytes);

    runtime.version = String(release.tag_name ?? runtime.release.tag).replace(/^v/, '');
    runtime.artifact = { url, sha256: hashed.sha256, size_bytes: hashed.size_bytes };
  }
}

async function provisionVoices(manifest) {
  console.log('\nResolving voices…');
  for (const voice of manifest.voices) {
    for (const field of ['model', 'config']) {
      const artifact = voice[field];
      if (!artifact?.url) fail(`Voice ${voice.id} has no ${field} URL`);
      const hashed = await fetchAndHash(artifact.url, `${voice.id} ${field}`);
      artifact.sha256 = hashed.sha256;
      artifact.size_bytes = hashed.size_bytes;
    }
  }
}

async function main() {
  if (process.env.RELAY_GITHUB_API) {
    console.warn(
      `\n!  Reading release metadata from ${process.env.RELAY_GITHUB_API}, not GitHub.\n` +
        '   Artifacts are still downloaded from the URL the manifest pins, and\n' +
        '   still hashed and opened. Unset RELAY_GITHUB_API for a real release.',
    );
  }

  const raw = await readFile(MANIFEST, 'utf8');
  const manifest = JSON.parse(raw);

  if (manifest.schema_version !== SCHEMA_VERSION) {
    fail(
      `The manifest is schema_version ${manifest.schema_version}, and this ` +
        `script writes ${SCHEMA_VERSION}. They are updated together.`,
    );
  }
  checkManifestShape(manifest);

  await provisionRuntimes(manifest);
  await provisionVoices(manifest);

  const next = `${JSON.stringify(manifest, null, 2)}\n`;

  if (checkOnly) {
    if (next !== raw) {
      fail('The manifest is out of date. Run without --check to update it.');
    }
    console.log('\n✓ Manifest is up to date and every artifact verified.\n');
    return;
  }

  await writeFile(MANIFEST, next, 'utf8');
  console.log(`\n✓ Wrote ${path.relative(ROOT, MANIFEST)}`);
  console.log('  Rebuild the Rust crate to compile the new checksums in.\n');
}

// Importable for tests; the CLI invocation is the only thing that runs.
export {
  archiveKindOf,
  checkManifestShape,
  downloadUrl,
  FatalError,
  findAsset,
  verifyRuntimeArchive,
};

/**
 * Runs `main`, reports any failure, and lets the process end on its own.
 *
 * Never `process.exit()`. Calling it from inside this flow tears the event
 * loop down while `fetch` still holds handles, and on Windows that aborts
 * the process with
 *
 *     Assertion failed: !(handle->flags & UV_HANDLE_CLOSING), src\win\async.c
 *
 * — which is how a plain "that asset does not exist" turned into a crash
 * report. Setting `exitCode` gives the shell the same answer, after the
 * message has actually been written.
 */
function run() {
  main().catch((error) => {
    if (lineIsOpen) endLine('');
    const message =
      error instanceof FatalError ? error.message : (error.stack ?? String(error));
    console.error(`\n✗ ${message}\n`);
    process.exitCode = 1;
  });
}

if (process.argv[1] && pathToFileURL(process.argv[1]).href === import.meta.url) {
  run();
}
