const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const defaultRootDir = path.resolve(__dirname, '..');

function getManifestPaths(root = defaultRootDir) {
  return {
    version: path.join(root, 'VERSION'),
    changelog: path.join(root, 'CHANGELOG.md'),
    rootPkg: path.join(root, 'package.json'),
    nativePkg: path.join(root, 'native', 'package.json'),
    tauriConf: path.join(root, 'native', 'src-tauri', 'tauri.conf.json'),
    cargoToml: path.join(root, 'native', 'src-tauri', 'Cargo.toml'),
    readme: path.join(root, 'README.md')
  };
}

function verifyManifests(root = defaultRootDir) {
  const paths = getManifestPaths(root);

  if (!fs.existsSync(paths.version)) {
    throw new Error('VERSION file is missing at repo root.');
  }

  const version = fs.readFileSync(paths.version, 'utf8').trim();
  if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`Invalid version format in VERSION file: "${version}"`);
  }

  // Verify native/src-tauri/tauri.conf.json
  if (fs.existsSync(paths.tauriConf)) {
    const tauriConf = JSON.parse(fs.readFileSync(paths.tauriConf, 'utf8'));
    if (tauriConf.version !== version) {
      throw new Error(`Version mismatch in native/src-tauri/tauri.conf.json (found "${tauriConf.version}", expected "${version}").`);
    }
  }

  // Verify native/package.json
  if (fs.existsSync(paths.nativePkg)) {
    const nativePkg = JSON.parse(fs.readFileSync(paths.nativePkg, 'utf8'));
    if (nativePkg.version !== version) {
      throw new Error(`Version mismatch in native/package.json (found "${nativePkg.version}", expected "${version}").`);
    }
  }

  // Verify root package.json
  if (fs.existsSync(paths.rootPkg)) {
    const rootPkg = JSON.parse(fs.readFileSync(paths.rootPkg, 'utf8'));
    if (rootPkg.version !== version) {
      throw new Error(`Version mismatch in root package.json (found "${rootPkg.version}", expected "${version}").`);
    }
  }

  // Verify native/src-tauri/Cargo.toml
  if (fs.existsSync(paths.cargoToml)) {
    const cargoToml = fs.readFileSync(paths.cargoToml, 'utf8');
    const match = cargoToml.match(/\[package\][\s\S]*?version\s*=\s*"([^"]+)"/);
    if (!match || match[1] !== version) {
      throw new Error(`Version mismatch in native/src-tauri/Cargo.toml (found "${match ? match[1] : 'unknown'}", expected "${version}").`);
    }
  }

  return version;
}

function verifyVersionAndChangelog(options = {}) {
  const mode = options.mode || 'dev';
  const root = options.rootDir || defaultRootDir;
  const paths = getManifestPaths(root);

  const version = verifyManifests(root);

  if (!fs.existsSync(paths.changelog)) {
    throw new Error('CHANGELOG.md file is missing at repo root.');
  }

  const changelogContent = fs.readFileSync(paths.changelog, 'utf8');
  if (!changelogContent.trim()) {
    throw new Error('CHANGELOG.md file is empty.');
  }

  if (mode === 'release') {
    // Release mode: VERSION must match the topmost release entry in CHANGELOG.md
    const topHeaderMatch = changelogContent.match(/^##\s*\[(\d+\.\d+\.\d+)\]/m);
    if (!topHeaderMatch) {
      throw new Error(`CHANGELOG.md contains no release entries of the form "## [x.y.z]".`);
    }

    if (topHeaderMatch[1] !== version) {
      throw new Error(`CHANGELOG.md topmost release entry is [${topHeaderMatch[1]}], but VERSION is ${version}. Release changelog must be updated with an entry for ${version}.`);
    }
    console.log(`✅ Release validation passed: v${version} verified in manifests and CHANGELOG.md.`);
  } else {
    // Development mode:
    // 1. VERSION represents latest released version; development commits do NOT add release entries.
    // 2. Guard against accidental agent-owned version bumps during dev commits.
    if (!options.allowVersionChange && !process.env.RELAY_RELEASE_RUN) {
      try {
        const stagedFiles = execSync('git diff --cached --name-only', {
          encoding: 'utf8',
          stdio: ['pipe', 'pipe', 'ignore'],
          cwd: root
        });
        const stagedList = stagedFiles.split('\n').map(f => f.trim()).filter(Boolean);
        if (stagedList.includes('VERSION')) {
          throw new Error('Development tasks MUST NOT modify the VERSION file. Versioning is owned exclusively by the release pipeline (.github/workflows/release.yml per rules/version-and-changelog.md).');
        }
      } catch (err) {
        if (err.message && err.message.includes('Development tasks MUST NOT modify')) {
          throw err;
        }
        // Git command failed or not in a git working tree — ignore staged diff check
      }
    }
    console.log(`✅ Development validation passed: manifests synchronized at v${version} (release-owned).`);
  }

  return version;
}

function verifyReadme(root = defaultRootDir) {
  const paths = getManifestPaths(root);
  if (!fs.existsSync(paths.readme)) {
    throw new Error('README.md is missing.');
  }

  const readmeContent = fs.readFileSync(paths.readme, 'utf8');
  const lines = readmeContent.split('\n');

  // Remove code blocks prior to checking Markdown headers
  const contentWithoutCodeBlocks = readmeContent.replace(/```[\s\S]*?```/g, '');

  // Check 1: Single H1 title
  const h1Count = (contentWithoutCodeBlocks.match(/^#\s+/gm) || []).length;
  if (h1Count !== 1) {
    throw new Error(`README.md MUST have exactly one H1 title. Found ${h1Count}.`);
  }

  // Check 2: Tagline formatted as blockquote
  const hasTaglineBlockquote = lines.some(line => line.trim().startsWith('>'));
  if (!hasTaglineBlockquote) {
    throw new Error('README.md MUST have a tagline formatted as a blockquote ("> ...") under the title per rules/readme.md.');
  }

  // Check 3: Fenced code blocks language tags (opening fences must have a language specifier)
  let inCodeBlock = false;
  let missingLang = false;
  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith('```')) {
      if (!inCodeBlock) {
        const lang = trimmed.slice(3).trim();
        if (!lang) {
          missingLang = true;
          break;
        }
        inCodeBlock = true;
      } else {
        inCodeBlock = false;
      }
    }
  }

  if (missingLang) {
    throw new Error('README.md has untagged code blocks. Every fenced code block MUST specify a language tag per rules/readme.md.');
  }

  console.log('✅ README.md verified against rules/readme.md');
}

function parseArgs(argv = process.argv.slice(2)) {
  let mode = 'dev';
  let allowVersionChange = false;

  for (const arg of argv) {
    if (arg === '--mode=release' || arg === '--release') {
      mode = 'release';
    } else if (arg === '--mode=dev' || arg === '--dev') {
      mode = 'dev';
    } else if (arg === '--allow-version-change') {
      allowVersionChange = true;
    }
  }

  if (process.env.RELAY_VERIFY_MODE) {
    mode = process.env.RELAY_VERIFY_MODE;
  }

  return { mode, allowVersionChange };
}

function main() {
  const args = parseArgs();
  console.log(`🔍 Running Relay Rule Verification [mode: ${args.mode}]...`);

  try {
    verifyVersionAndChangelog(args);
    verifyReadme();
    console.log('🎉 All repository rules verified successfully!');
  } catch (err) {
    console.error(`❌ ERROR: ${err.message}`);
    process.exit(1);
  }
}

if (require.main === module) {
  main();
}

module.exports = {
  getManifestPaths,
  verifyManifests,
  verifyVersionAndChangelog,
  verifyReadme,
  parseArgs
};
