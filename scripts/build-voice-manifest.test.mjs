/**
 * Tests for the release step that provisions Relay's voice catalogue.
 *
 * The bug these exist for: the manifest pointed at a project that had
 * stopped publishing an executable, and the generator's only defence was a
 * filename pattern. It failed with "no asset for piper-windows-x86_64",
 * which reads like a naming problem and invites the one fix that must not
 * be made — renaming the expected asset to whatever the release happens to
 * contain. `piper_tts-1.7.0-cp39-abi3-win_amd64.whl` would then have
 * downloaded, hashed, pinned and shipped cleanly, and no user would ever
 * have heard a word.
 *
 * So the cases below are mostly about what the generator *refuses*, and
 * about whether the refusal explains itself.
 *
 *     node --test scripts/
 */

import assert from 'node:assert/strict';
import { describe, it } from 'node:test';

import {
  ArchiveError,
  describeContents,
  indexArchive,
  indexTar,
  indexTarGz,
  indexZip,
  requireExecutable,
} from './lib/archive-index.mjs';
import { makeTar, makeTarGz, makeZip } from './lib/fake-archives.mjs';
import {
  archiveKindOf,
  checkManifestShape,
  downloadUrl,
  FatalError,
  findAsset,
  verifyRuntimeArchive,
} from './build-voice-manifest.mjs';

const ENGINE_ZIP = [
  { name: 'piper/', directory: true },
  { name: 'piper/piper.exe', body: Buffer.from('MZ this is a binary') },
  { name: 'piper/onnxruntime.dll', body: Buffer.from('a library') },
  { name: 'piper/espeak-ng-data/phontab', body: Buffer.from('data') },
];

/** The shape of the artifact that started all this. */
const WHEEL_ZIP = [
  { name: 'piper/__init__.py', body: Buffer.from('# python') },
  { name: 'piper/voice.py', body: Buffer.from('# python') },
  { name: 'piper/espeakbridge.pyd', body: Buffer.from('an extension module') },
  { name: 'piper_tts-1.7.0.dist-info/METADATA', body: Buffer.from('Name: piper-tts') },
];

const ENGINE_TAR = [
  { name: 'piper/piper', body: Buffer.from('\x7fELF a binary'), mode: 0o755 },
  { name: 'piper/libonnxruntime.so.1.14.1', body: Buffer.from('a library'), mode: 0o644 },
  { name: 'piper/libonnxruntime.so', type: '2', link: 'libonnxruntime.so.1.14.1', mode: 0o777 },
];

function runtime(overrides = {}) {
  return {
    id: 'piper-windows-x86_64',
    engine: 'piper',
    version: '',
    platform: 'windows',
    arch: 'x86_64',
    archive: 'zip',
    executable_path: 'piper/piper.exe',
    release: { repo: 'rhasspy/piper', tag: '2023.11.14-2', asset: 'piper_windows_amd64.zip' },
    artifact: { url: '', sha256: '', size_bytes: 0 },
    license: 'MIT',
    source: 'https://github.com/rhasspy/piper',
    ...overrides,
  };
}

function manifest(runtimes = [runtime()]) {
  return {
    schema_version: 2,
    runtimes,
    voices: [{ id: 'en_US-amy-medium', recommended: true }],
  };
}

describe('reading a zip', () => {
  it('lists every entry from the central directory', () => {
    const entries = indexZip(makeZip(ENGINE_ZIP));
    assert.equal(entries.size, 4);
    assert.equal(entries.get('piper/piper.exe').size, 19);
    assert.equal(entries.get('piper').isDirectory, true);
  });

  it('finds the executable the manifest promises', () => {
    const entry = requireExecutable(indexZip(makeZip(ENGINE_ZIP)), 'piper/piper.exe', {
      requireExecutableBit: false,
    });
    assert.equal(entry.path, 'piper/piper.exe');
  });

  it('does not demand an executable bit a Windows zip cannot carry', () => {
    // A zip built on Windows records no Unix modes at all. Requiring one
    // would reject the real artifact.
    const entries = indexZip(makeZip(ENGINE_ZIP));
    assert.equal(entries.get('piper/piper.exe').mode, null);
    assert.doesNotThrow(() =>
      requireExecutable(entries, 'piper/piper.exe', { requireExecutableBit: true }),
    );
  });

  it('rejects something that is not a zip at all', () => {
    assert.throws(() => indexZip(Buffer.from('<html>404 Not Found</html>')), ArchiveError);
  });
});

describe('reading a tar.gz', () => {
  it('lists entries with their modes and links', () => {
    const entries = indexTarGz(makeTarGz(ENGINE_TAR));
    assert.equal(entries.get('piper/piper').mode, 0o755);
    assert.equal(entries.get('piper/libonnxruntime.so').isLink, true);
    assert.equal(entries.get('piper/libonnxruntime.so').linkTarget, 'libonnxruntime.so.1.14.1');
  });

  it('requires the executable bit, which a tar does carry', () => {
    const limp = makeTarGz([{ name: 'piper/piper', body: Buffer.from('ELF'), mode: 0o644 }]);
    assert.throws(
      () => requireExecutable(indexTarGz(limp), 'piper/piper', { requireExecutableBit: true }),
      /not marked executable/,
    );
  });

  it('reads a GNU long name', () => {
    const deep = `piper/${'nested/'.repeat(20)}piper`;
    const entries = indexTar(makeTar([{ name: deep, body: Buffer.from('ELF'), mode: 0o755, longName: true }]));
    assert.ok(entries.has(deep), `long name lost: ${[...entries.keys()]}`);
  });

  it('rejects something that is not a gzip stream', () => {
    assert.throws(() => indexTarGz(Buffer.from('not gzipped')), ArchiveError);
  });

  it('rejects a tar whose headers are not ustar', () => {
    assert.throws(() => indexTar(Buffer.alloc(512, 0x41)), ArchiveError);
  });
});

describe('an archive that is not an engine', () => {
  it('is refused, however well-formed it is', () => {
    // The wheel hashes, extracts and contains nothing runnable. Only
    // listing its entries can tell it apart from an engine.
    const entries = indexZip(makeZip(WHEEL_ZIP));
    assert.throws(
      () => requireExecutable(entries, 'piper/piper.exe', { requireExecutableBit: false }),
      /has no piper\/piper\.exe/,
    );
  });

  it('is described as a Python package rather than a missing file', () => {
    // "piper.exe is missing" and "this is a wheel" call for completely
    // different fixes, and only one of them is renaming something.
    const description = describeContents(indexZip(makeZip(WHEEL_ZIP)));
    assert.match(description, /Python file/);
    assert.match(description, /wheel/);
  });

  it('is refused by the runtime check with the artifact named', () => {
    assert.throws(
      () => verifyRuntimeArchive(runtime(), makeZip(WHEEL_ZIP)),
      (error) =>
        error instanceof FatalError &&
        /piper_windows_amd64\.zip is not an engine Relay can run/.test(error.message),
    );
  });

  it('is refused when the bytes are not the declared format', () => {
    assert.throws(
      () => verifyRuntimeArchive(runtime({ archive: 'tar_gz' }), makeZip(ENGINE_ZIP)),
      /could not be read as tar_gz/,
    );
  });

  it('accepts a real engine archive of either kind', () => {
    assert.doesNotThrow(() => verifyRuntimeArchive(runtime(), makeZip(ENGINE_ZIP)));
    assert.doesNotThrow(() =>
      verifyRuntimeArchive(
        runtime({ archive: 'tar_gz', executable_path: 'piper/piper', release: { repo: 'rhasspy/piper', tag: '2023.11.14-2', asset: 'piper_linux_x86_64.tar.gz' } }),
        makeTarGz(ENGINE_TAR),
      ),
    );
  });

  it('has no reader for a kind it does not implement', () => {
    assert.throws(() => indexArchive('7z', Buffer.alloc(0)), ArchiveError);
  });
});

describe('resolving the pinned asset', () => {
  const release = (names) => ({
    tag_name: '2023.11.14-2',
    assets: names.map((name) => ({
      name,
      browser_download_url: `https://github.com/rhasspy/piper/releases/download/2023.11.14-2/${name}`,
    })),
  });

  it('takes the asset by its exact name', () => {
    const asset = findAsset(
      release(['piper_linux_x86_64.tar.gz', 'piper_windows_amd64.zip']),
      runtime(),
    );
    assert.equal(asset.name, 'piper_windows_amd64.zip');
  });

  it('does not settle for a near miss', () => {
    // Nothing in this release is the pinned asset. A pattern would have
    // matched the first entry; a name lookup does not.
    assert.throws(
      () => findAsset(release(['piper_windows_amd64_v2.zip']), runtime()),
      /has no asset called piper_windows_amd64\.zip/,
    );
  });

  it('explains a release that publishes only Python distributions', () => {
    // This is the exact asset list of OHF-Voice/piper1-gpl v1.7.0.
    const wheels = release([
      'piper_tts-1.7.0-cp39-abi3-macosx_10_9_x86_64.whl',
      'piper_tts-1.7.0-cp39-abi3-macosx_11_0_arm64.whl',
      'piper_tts-1.7.0-cp39-abi3-manylinux_2_17_aarch64.manylinux2014_aarch64.whl',
      'piper_tts-1.7.0-cp39-abi3-manylinux_2_17_x86_64.manylinux2014_x86_64.whl',
      'piper_tts-1.7.0-cp39-abi3-win_amd64.whl',
      'piper_tts-1.7.0.tar.gz',
    ]);
    assert.throws(
      () => findAsset(wheels, runtime()),
      (error) => {
        assert.match(error.message, /Every asset here is a Python distribution/);
        assert.match(error.message, /Do not rename the expected asset/);
        // And it still shows what is actually there, so the reader can
        // check the claim rather than take it on trust.
        assert.match(error.message, /piper_tts-1\.7\.0-cp39-abi3-win_amd64\.whl/);
        return true;
      },
    );
  });
});

describe('the manifest the generator will fill in', () => {
  it('accepts the shape Relay ships', () => {
    assert.doesNotThrow(() => checkManifestShape(manifest()));
  });

  it('refuses a wheel named as a runtime artifact', () => {
    const broken = manifest([
      runtime({
        release: { repo: 'OHF-Voice/piper1-gpl', tag: 'v1.7.0', asset: 'piper_tts-1.7.0-cp39-abi3-win_amd64.whl' },
        source: 'https://github.com/OHF-Voice/piper1-gpl',
      }),
    ]);
    assert.throws(() => checkManifestShape(broken), /Python wheel/);
  });

  it('refuses an archive kind that disagrees with the asset', () => {
    const broken = manifest([
      runtime({ release: { repo: 'rhasspy/piper', tag: '2023.11.14-2', asset: 'piper_linux_x86_64.tar.gz' } }),
    ]);
    assert.throws(() => checkManifestShape(broken), /declared "zip"/);
  });

  it('refuses an executable path that could escape the engine folder', () => {
    for (const bad of ['../piper.exe', '/usr/bin/piper', 'C:/piper.exe', 'piper\\piper.exe', '']) {
      assert.throws(
        () => checkManifestShape(manifest([runtime({ executable_path: bad })])),
        FatalError,
        `accepted ${JSON.stringify(bad)}`,
      );
    }
  });

  it('refuses a source that is not the project the artifact comes from', () => {
    const broken = manifest([runtime({ source: 'https://github.com/OHF-Voice/piper1-gpl' })]);
    assert.throws(() => checkManifestShape(broken), /source is not the project/);
  });

  it('refuses a release pin that is not a plain repo, tag and asset', () => {
    for (const release of [
      { repo: 'piper', tag: '1', asset: 'a.zip' },
      { repo: 'rhasspy/piper', tag: '../../x', asset: 'a.zip' },
      { repo: 'rhasspy/piper', tag: '1', asset: 'nested/a.zip' },
      { repo: 'rhasspy/piper', tag: '', asset: 'a.zip' },
    ]) {
      assert.throws(
        () => checkManifestShape(manifest([runtime({ release })])),
        FatalError,
        `accepted ${JSON.stringify(release)}`,
      );
    }
  });

  it('requires exactly one recommended voice', () => {
    const none = manifest();
    none.voices[0].recommended = false;
    assert.throws(() => checkManifestShape(none), /exactly one voice must be recommended/);
  });
});

describe('the download URL', () => {
  it('is derived from the pin, not taken from an API response', () => {
    assert.equal(
      downloadUrl({ repo: 'rhasspy/piper', tag: '2023.11.14-2', asset: 'piper_windows_amd64.zip' }),
      'https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip',
    );
  });

  it('maps asset names to the kinds Relay can unpack', () => {
    assert.equal(archiveKindOf('piper_windows_amd64.zip'), 'zip');
    assert.equal(archiveKindOf('piper_linux_x86_64.tar.gz'), 'tar_gz');
    assert.equal(archiveKindOf('piper.tgz'), 'tar_gz');
    assert.equal(archiveKindOf('piper_tts-1.7.0-cp39-abi3-win_amd64.whl'), null);
    assert.equal(archiveKindOf('piper.tar.bz2'), null);
  });
});
