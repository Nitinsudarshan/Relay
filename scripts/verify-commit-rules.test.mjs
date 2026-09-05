import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  verifyManifests,
  verifyVersionAndChangelog,
  verifyReadme,
  parseArgs
} from './verify-commit-rules.js';

function createTempRepo(version = '1.2.3', changelogContent = null) {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'relay-verify-test-'));

  // 1. VERSION
  fs.writeFileSync(path.join(tmpDir, 'VERSION'), `${version}\n`, 'utf8');

  // 2. package.json
  fs.writeFileSync(
    path.join(tmpDir, 'package.json'),
    JSON.stringify({ name: 'relay-root', version }, null, 2),
    'utf8'
  );

  // 3. native/package.json
  fs.mkdirSync(path.join(tmpDir, 'native', 'src-tauri'), { recursive: true });
  fs.writeFileSync(
    path.join(tmpDir, 'native', 'package.json'),
    JSON.stringify({ name: 'relay-native', version }, null, 2),
    'utf8'
  );

  // 4. native/src-tauri/tauri.conf.json
  fs.writeFileSync(
    path.join(tmpDir, 'native', 'src-tauri', 'tauri.conf.json'),
    JSON.stringify({ productName: 'Relay', version }, null, 2),
    'utf8'
  );

  // 5. native/src-tauri/Cargo.toml
  fs.writeFileSync(
    path.join(tmpDir, 'native', 'src-tauri', 'Cargo.toml'),
    `[package]\nname = "relay-native-backend"\nversion = "${version}"\nedition = "2021"\n`,
    'utf8'
  );

  // CHANGELOG.md
  const defaultChangelog = changelogContent ?? `# Relay — Changelog\n\n## [${version}] - 2026-09-06\n\n### Features\n- Initial feature\n`;
  fs.writeFileSync(path.join(tmpDir, 'CHANGELOG.md'), defaultChangelog, 'utf8');

  // README.md
  fs.writeFileSync(
    path.join(tmpDir, 'README.md'),
    `# Relay\n\n> The AI-assisted workspace\n\n\`\`\`bash\nnpm install\n\`\`\`\n`,
    'utf8'
  );

  return tmpDir;
}

test('parseArgs recognizes dev and release modes', () => {
  assert.deepEqual(parseArgs([]), { mode: 'dev', allowVersionChange: false });
  assert.deepEqual(parseArgs(['--mode=release']), { mode: 'release', allowVersionChange: false });
  assert.deepEqual(parseArgs(['--release']), { mode: 'release', allowVersionChange: false });
  assert.deepEqual(parseArgs(['--mode=dev', '--allow-version-change']), {
    mode: 'dev',
    allowVersionChange: true
  });
});

test('verifyManifests passes when all 5 files agree on version', () => {
  const tmp = createTempRepo('0.41.0');
  try {
    const verified = verifyManifests(tmp);
    assert.equal(verified, '0.41.0');
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});

test('verifyManifests fails when Cargo.toml disagrees', () => {
  const tmp = createTempRepo('0.41.0');
  try {
    fs.writeFileSync(
      path.join(tmp, 'native', 'src-tauri', 'Cargo.toml'),
      `[package]\nname = "relay-native-backend"\nversion = "0.40.0"\nedition = "2021"\n`,
      'utf8'
    );
    assert.throws(() => verifyManifests(tmp), /Version mismatch in native\/src-tauri\/Cargo\.toml/);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});

test('verifyManifests fails when native/package.json disagrees', () => {
  const tmp = createTempRepo('0.41.0');
  try {
    fs.writeFileSync(
      path.join(tmp, 'native', 'package.json'),
      JSON.stringify({ name: 'relay-native', version: '0.40.0' }),
      'utf8'
    );
    assert.throws(() => verifyManifests(tmp), /Version mismatch in native\/package\.json/);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});

test('verifyVersionAndChangelog in dev mode does not require a new entry for unreleased tasks', () => {
  // Current released version in VERSION is 0.41.0, CHANGELOG top entry is 0.41.0
  const tmp = createTempRepo('0.41.0');
  try {
    const res = verifyVersionAndChangelog({ mode: 'dev', rootDir: tmp });
    assert.equal(res, '0.41.0');
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});

test('verifyVersionAndChangelog in release mode requires matching topmost changelog entry', () => {
  // VERSION = 0.42.0 but changelog top is 0.41.0
  const tmp = createTempRepo(
    '0.42.0',
    '# Relay — Changelog\n\n## [0.41.0] - 2026-09-05\n\n### Fixes\n- old fix\n'
  );
  try {
    assert.throws(
      () => verifyVersionAndChangelog({ mode: 'release', rootDir: tmp }),
      /CHANGELOG\.md topmost release entry is \[0\.41\.0\], but VERSION is 0\.42\.0/
    );

    // Now update CHANGELOG to include 0.42.0 as topmost
    fs.writeFileSync(
      path.join(tmp, 'CHANGELOG.md'),
      '# Relay — Changelog\n\n## [0.42.0] - 2026-09-06\n\n### Fixes\n- new fix\n\n## [0.41.0] - 2026-09-05\n'
    );
    const res = verifyVersionAndChangelog({ mode: 'release', rootDir: tmp });
    assert.equal(res, '0.42.0');
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});

test('verifyReadme checks H1, tagline blockquote, and code block language tags', () => {
  const tmp = createTempRepo('0.41.0');
  try {
    assert.doesNotThrow(() => verifyReadme(tmp));

    // Missing language in code block
    fs.writeFileSync(
      path.join(tmp, 'README.md'),
      `# Relay\n\n> The AI-assisted workspace\n\n\`\`\`\nmissing-lang\n\`\`\`\n`
    );
    assert.throws(() => verifyReadme(tmp), /Every fenced code block MUST specify a language tag/);
  } finally {
    fs.rmSync(tmp, { recursive: true, force: true });
  }
});
