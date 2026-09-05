import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  parseConventionalCommit,
  categorizeCommits,
  bumpVersion,
  determineBumpType,
  formatChangelogEntry,
  updateManifests,
  prependChangelog
} from './prepare-release.mjs';

test('parseConventionalCommit parses various conventional commit patterns', () => {
  const feat = parseConventionalCommit('feat(meetings): add calendar sync');
  assert.deepEqual(feat, {
    type: 'feat',
    scope: 'meetings',
    isBreaking: false,
    description: 'add calendar sync',
    body: ''
  });

  const fix = parseConventionalCommit('fix: handle null participant pointer\n\nMore details');
  assert.deepEqual(fix, {
    type: 'fix',
    scope: null,
    isBreaking: false,
    description: 'handle null participant pointer',
    body: 'More details'
  });

  const breakingBang = parseConventionalCommit('feat(api)!: drop legacy rest endpoints');
  assert.equal(breakingBang.isBreaking, true);
  assert.equal(breakingBang.type, 'feat');

  const breakingFooter = parseConventionalCommit(
    'refactor: overhaul storage schema\n\nBREAKING CHANGE: sqlite table columns renamed'
  );
  assert.equal(breakingFooter.isBreaking, true);

  assert.equal(parseConventionalCommit('not a conventional commit'), null);
});

test('bumpVersion handles patch, minor, major correctly', () => {
  assert.equal(bumpVersion('0.41.0', 'patch'), '0.41.1');
  assert.equal(bumpVersion('0.41.0', 'minor'), '0.42.0');
  assert.equal(bumpVersion('0.41.0', 'major'), '1.0.0');
  assert.throws(() => bumpVersion('invalid', 'patch'), /Invalid semver/);
});

test('categorizeCommits and determineBumpType group commits and decide version bump', () => {
  const commits = [
    { message: 'feat(calendar): sync meetings' },
    { message: 'fix(recorder): prevent clip' },
    { message: 'refactor(vault): clean index' }
  ];

  const categories = categorizeCommits(commits);
  assert.equal(categories.features.length, 1);
  assert.equal(categories.fixes.length, 1);
  assert.equal(categories.improvements.length, 1);
  assert.equal(categories.breaking.length, 0);

  assert.equal(determineBumpType(categories), 'minor');

  // If breaking change exists
  const breakingCommits = [
    ...commits,
    { message: 'feat!: redesign public command signatures' }
  ];
  const breakingCats = categorizeCommits(breakingCommits);
  assert.equal(determineBumpType(breakingCats), 'major');

  // If only fixes exist
  const fixOnlyCats = categorizeCommits([{ message: 'fix: typo in label' }]);
  assert.equal(determineBumpType(fixOnlyCats), 'patch');
});

test('formatChangelogEntry produces structured markdown', () => {
  const categories = {
    breaking: [],
    features: [{ scope: 'meetings', description: 'add calendar sync', hash: 'abc1234' }],
    improvements: [{ scope: null, description: 'clean index', hash: 'def5678' }],
    fixes: [{ scope: 'recorder', description: 'prevent clip', hash: '789abcd' }],
    security: [],
    other: []
  };

  const md = formatChangelogEntry('0.42.0', '2026-09-06', categories);
  assert.match(md, /## \[0\.42\.0\] - 2026-09-06/);
  assert.match(md, /### Features/);
  assert.match(md, /\*\*meetings\*\*: add calendar sync/);
  assert.match(md, /### Fixes/);
  assert.match(md, /\*\*recorder\*\*: prevent clip/);
  assert.match(md, /### Improvements/);
});

test('updateManifests atomically updates all 5 manifests', () => {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'relay-manifest-test-'));
  try {
    fs.writeFileSync(path.join(tmpDir, 'VERSION'), '0.41.0\n', 'utf8');
    fs.writeFileSync(
      path.join(tmpDir, 'package.json'),
      JSON.stringify({ name: 'relay', version: '0.41.0' }, null, 2),
      'utf8'
    );
    fs.mkdirSync(path.join(tmpDir, 'native', 'src-tauri'), { recursive: true });
    fs.writeFileSync(
      path.join(tmpDir, 'native', 'package.json'),
      JSON.stringify({ name: 'relay-native', version: '0.41.0' }, null, 2),
      'utf8'
    );
    fs.writeFileSync(
      path.join(tmpDir, 'native', 'src-tauri', 'tauri.conf.json'),
      JSON.stringify({ productName: 'Relay', version: '0.41.0' }, null, 2),
      'utf8'
    );
    fs.writeFileSync(
      path.join(tmpDir, 'native', 'src-tauri', 'Cargo.toml'),
      '[package]\nname = "relay-native-backend"\nversion = "0.41.0"\nedition = "2021"\n',
      'utf8'
    );

    updateManifests('0.42.0', tmpDir);

    assert.equal(fs.readFileSync(path.join(tmpDir, 'VERSION'), 'utf8').trim(), '0.42.0');
    assert.equal(JSON.parse(fs.readFileSync(path.join(tmpDir, 'package.json'), 'utf8')).version, '0.42.0');
    assert.equal(
      JSON.parse(fs.readFileSync(path.join(tmpDir, 'native', 'package.json'), 'utf8')).version,
      '0.42.0'
    );
    assert.equal(
      JSON.parse(fs.readFileSync(path.join(tmpDir, 'native', 'src-tauri', 'tauri.conf.json'), 'utf8'))
        .version,
      '0.42.0'
    );
    const cargo = fs.readFileSync(path.join(tmpDir, 'native', 'src-tauri', 'Cargo.toml'), 'utf8');
    assert.match(cargo, /version = "0\.42\.0"/);
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
});

test('prependChangelog preserves header and prepends new entry', () => {
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'relay-changelog-test-'));
  try {
    const original = '# Relay — Changelog\n\n## [0.41.0] - 2026-09-05\n\n### Fixes\n- old fix\n';
    fs.writeFileSync(path.join(tmpDir, 'CHANGELOG.md'), original, 'utf8');

    const newEntry = '## [0.42.0] - 2026-09-06\n\n### Features\n- new feature\n';
    prependChangelog(newEntry, tmpDir);

    const updated = fs.readFileSync(path.join(tmpDir, 'CHANGELOG.md'), 'utf8');
    assert.match(updated, /^# Relay — Changelog\n\n## \[0\.42\.0\] - 2026-09-06[\s\S]*## \[0\.41\.0\]/);
  } finally {
    fs.rmSync(tmpDir, { recursive: true, force: true });
  }
});
