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
 *     node scripts/build-voice-manifest.mjs --check    # verify, don't write
 *     node scripts/build-voice-manifest.mjs --piper-tag v1.6.0
 *
 * It resolves the Piper release assets from the GitHub API rather than
 * from a hardcoded filename, downloads every artifact, hashes it, and
 * rewrites the manifest in place. It never invents a value: if an artifact
 * cannot be fetched, the run fails and the manifest is left untouched.
 */

import { createHash } from 'node:crypto';
import { readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const MANIFEST = path.join(ROOT, 'native/src-tauri/resources/voice-manifest.json');

const PIPER_REPO = 'OHF-Voice/piper1-gpl';

/** Which release asset belongs to which runtime entry. */
const RUNTIME_ASSET_MATCHERS = {
  'piper-windows-x86_64': (name) =>
    /windows/i.test(name) && /(x86_64|amd64|x64)/i.test(name) && name.endsWith('.zip'),
  'piper-linux-x86_64': (name) =>
    /linux/i.test(name) && /(x86_64|amd64)/i.test(name) && /\.(zip|tar\.gz)$/i.test(name),
};

const args = process.argv.slice(2);
const checkOnly = args.includes('--check');
const pinnedTag = valueOf('--piper-tag');

function valueOf(flag) {
  const index = args.indexOf(flag);
  return index >= 0 ? args[index + 1] : undefined;
}

function fail(message) {
  console.error(`\n✗ ${message}\n`);
  process.exit(1);
}

/** Downloads a URL fully, returning its bytes, sha256 and size. */
async function fetchAndHash(url, label) {
  process.stdout.write(`  ${label} … `);
  const response = await fetch(url, { redirect: 'follow' });
  if (!response.ok) {
    fail(`${label}: ${response.status} ${response.statusText}\n  ${url}`);
  }
  const bytes = Buffer.from(await response.arrayBuffer());
  if (bytes.length === 0) fail(`${label}: empty response from ${url}`);
  const sha256 = createHash('sha256').update(bytes).digest('hex');
  console.log(`${(bytes.length / 1e6).toFixed(1)} MB  ${sha256.slice(0, 12)}…`);
  return { sha256, size_bytes: bytes.length, url: response.url };
}

/** Resolves the Piper release and its downloadable assets. */
async function resolvePiperRelease() {
  const endpoint = pinnedTag
    ? `https://api.github.com/repos/${PIPER_REPO}/releases/tags/${pinnedTag}`
    : `https://api.github.com/repos/${PIPER_REPO}/releases/latest`;

  const response = await fetch(endpoint, {
    headers: {
      Accept: 'application/vnd.github+json',
      'User-Agent': 'relay-voice-manifest',
      // Optional, but avoids the unauthenticated rate limit in CI.
      ...(process.env.GITHUB_TOKEN
        ? { Authorization: `Bearer ${process.env.GITHUB_TOKEN}` }
        : {}),
    },
  });
  if (!response.ok) {
    fail(
      `Could not read the Piper release (${response.status}). ` +
        `Set GITHUB_TOKEN if you are being rate-limited.`,
    );
  }
  const release = await response.json();
  console.log(`Piper release: ${release.tag_name}`);
  return release;
}

/** The path of the executable inside the archive, as the release ships it. */
function executablePathFor(entry) {
  return entry.executable_path || (entry.platform === 'windows' ? 'piper/piper.exe' : 'piper/piper');
}

async function main() {
  const raw = await readFile(MANIFEST, 'utf8');
  const manifest = JSON.parse(raw);

  if (manifest.schema_version !== 1) {
    fail(`Unknown manifest schema_version ${manifest.schema_version}`);
  }

  console.log('\nResolving voice engine…');
  const release = await resolvePiperRelease();
  const version = String(release.tag_name ?? '').replace(/^v/, '');

  for (const runtime of manifest.runtimes) {
    const matcher = RUNTIME_ASSET_MATCHERS[runtime.id];
    if (!matcher) fail(`No asset matcher for runtime ${runtime.id}`);

    const asset = (release.assets ?? []).find((a) => matcher(a.name));
    if (!asset) {
      fail(
        `Release ${release.tag_name} has no asset for ${runtime.id}. ` +
          `Assets: ${(release.assets ?? []).map((a) => a.name).join(', ') || '(none)'}`,
      );
    }

    const hashed = await fetchAndHash(asset.browser_download_url, runtime.id);
    runtime.version = version;
    runtime.executable_path = executablePathFor(runtime);
    runtime.artifact = {
      url: asset.browser_download_url,
      sha256: hashed.sha256,
      size_bytes: hashed.size_bytes,
    };
  }

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

  // The Rust side enforces this too; failing here means a bad manifest
  // never reaches a commit.
  const recommended = manifest.voices.filter((v) => v.recommended);
  if (recommended.length !== 1) {
    fail(`Exactly one voice must be recommended; found ${recommended.length}`);
  }

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

main().catch((error) => fail(error.stack ?? String(error)));
