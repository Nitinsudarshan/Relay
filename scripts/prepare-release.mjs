#!/usr/bin/env node
/**
 * Relay Release Preparation Script
 *
 * Automates the release version bump, atomic manifest synchronization,
 * and release changelog generation from Conventional Commits.
 *
 * Usage:
 *   node scripts/prepare-release.mjs                     # Auto-detect bump from commits
 *   node scripts/prepare-release.mjs --bump=patch        # Force patch bump
 *   node scripts/prepare-release.mjs --bump=minor        # Force minor bump
 *   node scripts/prepare-release.mjs --bump=major        # Force major bump
 *   node scripts/prepare-release.mjs --version=0.42.0    # Set explicit version
 *   node scripts/prepare-release.mjs --dry-run           # Preview without modifying files
 */

import fs from 'fs';
import path from 'path';
import { execSync } from 'child_process';
import { fileURLToPath } from 'url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const defaultRootDir = path.resolve(__dirname, '..');

export function parseConventionalCommit(rawMessage) {
  if (!rawMessage || typeof rawMessage !== 'string') {
    return null;
  }

  const lines = rawMessage.trim().split('\n');
  const firstLine = lines[0].trim();
  const bodyAndFooter = lines.slice(1).join('\n');

  // Regex for <type>(<scope>)?: <description> or <type>!: <description>
  const match = firstLine.match(/^([a-zA-Z]+)(?:\(([^)]+)\))?(!)?:\s*(.+)$/);
  if (!match) {
    return null;
  }

  const [, rawType, scope, bang, description] = match;
  const type = rawType.toLowerCase();
  const isBreaking = Boolean(bang) || /BREAKING[\s-]CHANGE:/i.test(bodyAndFooter);

  return {
    type,
    scope: scope ? scope.trim() : null,
    isBreaking,
    description: description.trim(),
    body: bodyAndFooter.trim()
  };
}

export function categorizeCommits(commits) {
  const categories = {
    breaking: [],
    features: [],
    improvements: [],
    fixes: [],
    security: [],
    other: []
  };

  for (const commit of commits) {
    const parsed = parseConventionalCommit(commit.message || commit.subject);
    const item = {
      hash: commit.hash ? commit.hash.slice(0, 7) : '',
      scope: parsed?.scope || null,
      description: parsed ? parsed.description : (commit.subject || commit.message),
      raw: commit.message || commit.subject,
      isBreaking: parsed?.isBreaking || false
    };

    if (item.isBreaking) {
      categories.breaking.push(item);
      continue;
    }

    if (!parsed) {
      categories.other.push(item);
      continue;
    }

    switch (parsed.type) {
      case 'feat':
        categories.features.push(item);
        break;
      case 'fix':
        categories.fixes.push(item);
        break;
      case 'refactor':
      case 'perf':
      case 'style':
        categories.improvements.push(item);
        break;
      case 'sec':
      case 'security':
        categories.security.push(item);
        break;
      default:
        categories.other.push(item);
        break;
    }
  }

  return categories;
}

export function bumpVersion(currentVersion, bumpType = 'patch') {
  const parts = currentVersion.split('.').map(Number);
  if (parts.length !== 3 || parts.some(isNaN)) {
    throw new Error(`Invalid semver version: "${currentVersion}"`);
  }

  let [major, minor, patch] = parts;

  switch (bumpType) {
    case 'major':
      major += 1;
      minor = 0;
      patch = 0;
      break;
    case 'minor':
      minor += 1;
      patch = 0;
      break;
    case 'patch':
      patch += 1;
      break;
    default:
      throw new Error(`Unknown bump type: "${bumpType}"`);
  }

  return `${major}.${minor}.${patch}`;
}

export function determineBumpType(categories) {
  if (categories.breaking.length > 0) {
    return 'major';
  }
  if (categories.features.length > 0) {
    return 'minor';
  }
  return 'patch';
}

export function getCommitsSinceTag(tag, rootDir = defaultRootDir) {
  try {
    const range = tag ? `${tag}..HEAD` : 'HEAD';
    const output = execSync(
      `git log ${range} --pretty=format:"%H%x09%s%x09%b%x1e"`,
      { encoding: 'utf8', cwd: rootDir, stdio: ['pipe', 'pipe', 'ignore'] }
    );

    if (!output.trim()) {
      return [];
    }

    const records = output.split('\x1e');
    const commits = [];

    for (const record of records) {
      const trimmed = record.trim();
      if (!trimmed) continue;
      const [hash, subject, ...bodyParts] = trimmed.split('\x09');
      const body = bodyParts.join('\x09');
      const message = body ? `${subject}\n\n${body}` : subject;
      commits.push({ hash, subject, message });
    }

    return commits;
  } catch (err) {
    console.warn(`⚠️ Warning: Could not collect git commits (${err.message}).`);
    return [];
  }
}

export function getLatestReleaseTag(rootDir = defaultRootDir) {
  try {
    const tags = execSync('git tag -l "v*" --sort=-v:refname', {
      encoding: 'utf8',
      cwd: rootDir,
      stdio: ['pipe', 'pipe', 'ignore']
    })
      .split('\n')
      .map(t => t.trim())
      .filter(Boolean);

    return tags[0] || null;
  } catch {
    return null;
  }
}

export function formatChangelogEntry(version, dateStr, categories) {
  const sections = [];
  sections.push(`## [${version}] - ${dateStr}`);
  sections.push('');

  function renderCategory(title, items) {
    if (items.length === 0) return;
    sections.push(`### ${title}`);
    sections.push('');
    for (const item of items) {
      const scopePrefix = item.scope ? `**${item.scope}**: ` : '';
      const hashSuffix = item.hash ? ` (${item.hash})` : '';
      sections.push(`- ${scopePrefix}${item.description}${hashSuffix}`);
    }
    sections.push('');
  }

  renderCategory('Breaking Changes', categories.breaking);
  renderCategory('Features', categories.features);
  renderCategory('Improvements', categories.improvements);
  renderCategory('Fixes', categories.fixes);
  renderCategory('Security', categories.security);

  if (
    categories.breaking.length === 0 &&
    categories.features.length === 0 &&
    categories.improvements.length === 0 &&
    categories.fixes.length === 0 &&
    categories.security.length === 0
  ) {
    if (categories.other.length > 0) {
      renderCategory('Improvements', categories.other);
    } else {
      sections.push('### Improvements');
      sections.push('');
      sections.push('- Internal maintenance and updates.');
      sections.push('');
    }
  }

  return sections.join('\n');
}

export function updateManifests(newVersion, rootDir = defaultRootDir) {
  const versionPath = path.join(rootDir, 'VERSION');
  const rootPkgPath = path.join(rootDir, 'package.json');
  const nativePkgPath = path.join(rootDir, 'native', 'package.json');
  const tauriConfPath = path.join(rootDir, 'native', 'src-tauri', 'tauri.conf.json');
  const cargoTomlPath = path.join(rootDir, 'native', 'src-tauri', 'Cargo.toml');

  // 1. VERSION
  fs.writeFileSync(versionPath, `${newVersion}\n`, 'utf8');

  // 2. package.json
  if (fs.existsSync(rootPkgPath)) {
    const rootPkg = JSON.parse(fs.readFileSync(rootPkgPath, 'utf8'));
    rootPkg.version = newVersion;
    fs.writeFileSync(rootPkgPath, JSON.stringify(rootPkg, null, 2) + '\n', 'utf8');
  }

  // 3. native/package.json
  if (fs.existsSync(nativePkgPath)) {
    const nativePkg = JSON.parse(fs.readFileSync(nativePkgPath, 'utf8'));
    nativePkg.version = newVersion;
    fs.writeFileSync(nativePkgPath, JSON.stringify(nativePkg, null, 2) + '\n', 'utf8');
  }

  // 4. native/src-tauri/tauri.conf.json
  if (fs.existsSync(tauriConfPath)) {
    const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'));
    tauriConf.version = newVersion;
    fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n', 'utf8');
  }

  // 5. native/src-tauri/Cargo.toml
  if (fs.existsSync(cargoTomlPath)) {
    const cargoToml = fs.readFileSync(cargoTomlPath, 'utf8');
    const updated = cargoToml.replace(
      /(\[package\][\s\S]*?version\s*=\s*)"([^"]+)"/,
      `$1"${newVersion}"`
    );
    fs.writeFileSync(cargoTomlPath, updated, 'utf8');
  }
}

export function prependChangelog(newEntry, rootDir = defaultRootDir) {
  const changelogPath = path.join(rootDir, 'CHANGELOG.md');
  if (!fs.existsSync(changelogPath)) {
    fs.writeFileSync(changelogPath, `# Relay — Changelog\n\n${newEntry}\n`, 'utf8');
    return;
  }

  const existing = fs.readFileSync(changelogPath, 'utf8');
  const headerMatch = existing.match(/^#\s+[^\n]+\n+/);

  let updated;
  if (headerMatch) {
    const header = headerMatch[0];
    const rest = existing.slice(header.length);
    updated = `${header}${newEntry}\n${rest}`;
  } else {
    updated = `# Relay — Changelog\n\n${newEntry}\n${existing}`;
  }

  fs.writeFileSync(changelogPath, updated, 'utf8');
}

export function parseArgs(argv = process.argv.slice(2)) {
  let bump = null;
  let version = null;
  let dryRun = false;
  let fromTag = null;

  for (const arg of argv) {
    if (arg === '--dry-run') {
      dryRun = true;
    } else if (arg.startsWith('--bump=')) {
      bump = arg.split('=')[1].toLowerCase();
    } else if (arg.startsWith('--version=')) {
      version = arg.split('=')[1];
    } else if (arg.startsWith('--from=')) {
      fromTag = arg.split('=')[1];
    }
  }

  return { bump, version, dryRun, fromTag };
}

export async function prepareRelease(options = {}) {
  const rootDir = options.rootDir || defaultRootDir;
  const versionPath = path.join(rootDir, 'VERSION');

  if (!fs.existsSync(versionPath)) {
    throw new Error(`VERSION file not found at ${versionPath}`);
  }

  const currentVersion = fs.readFileSync(versionPath, 'utf8').trim();
  const fromTag = options.fromTag || getLatestReleaseTag(rootDir) || `v${currentVersion}`;

  console.log(`Current release version: v${currentVersion}`);
  console.log(`Collecting commits since tag: ${fromTag}`);

  const commits = getCommitsSinceTag(fromTag, rootDir);
  console.log(`Found ${commits.length} commits since ${fromTag}`);

  const categories = categorizeCommits(commits);
  let nextVersion;

  if (options.version) {
    nextVersion = options.version;
  } else {
    const bumpType = options.bump || determineBumpType(categories);
    nextVersion = bumpVersion(currentVersion, bumpType);
    console.log(`Determined bump type: ${bumpType} → v${nextVersion}`);
  }

  const today = new Date().toISOString().slice(0, 10);
  const changelogEntry = formatChangelogEntry(nextVersion, today, categories);

  console.log('\n--- Planned Changelog Entry ---');
  console.log(changelogEntry);
  console.log('-------------------------------\n');

  if (options.dryRun) {
    console.log('🔍 [DRY RUN] No files modified.');
    return {
      currentVersion,
      nextVersion,
      changelogEntry,
      categories,
      dryRun: true
    };
  }

  // Update manifests atomically
  console.log(`Synchronizing manifests to v${nextVersion}...`);
  updateManifests(nextVersion, rootDir);

  // Prepend to CHANGELOG.md
  console.log('Prepending release entry to CHANGELOG.md...');
  prependChangelog(changelogEntry, rootDir);

  console.log(`✨ Release v${nextVersion} prepared successfully!`);
  return {
    currentVersion,
    nextVersion,
    changelogEntry,
    categories,
    dryRun: false
  };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const args = parseArgs();
  prepareRelease(args).catch(err => {
    console.error(`❌ Release preparation failed: ${err.message}`);
    process.exit(1);
  });
}
