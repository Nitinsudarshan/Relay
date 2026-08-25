const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');

const rootDir = path.resolve(__dirname, '..');

function verifyVersionAndChangelog() {
  const versionPath = path.join(rootDir, 'VERSION');
  const changelogPath = path.join(rootDir, 'CHANGELOG.md');

  if (!fs.existsSync(versionPath)) {
    console.error('❌ ERROR: VERSION file is missing at repo root.');
    process.exit(1);
  }

  const version = fs.readFileSync(versionPath, 'utf8').trim();
  if (!version || !/^\d+\.\d+\.\d+$/.test(version)) {
    console.error(`❌ ERROR: Invalid version format in VERSION file: "${version}"`);
    process.exit(1);
  }

  if (!fs.existsSync(changelogPath)) {
    console.error('❌ ERROR: CHANGELOG.md file is missing at repo root.');
    process.exit(1);
  }

  const changelogContent = fs.readFileSync(changelogPath, 'utf8');
  if (!changelogContent.includes(`## [${version}]`)) {
    console.error(`❌ ERROR: CHANGELOG.md is missing an entry for the current VERSION (${version}).`);
    console.error('Please update CHANGELOG.md before committing or pushing per rules/version-and-changelog.md.');
    process.exit(1);
  }

  // Verify tauri.conf.json version consistency
  const tauriConfPath = path.join(rootDir, 'native', 'src-tauri', 'tauri.conf.json');
  if (fs.existsSync(tauriConfPath)) {
    const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'));
    if (tauriConf.version !== version) {
      console.error(`❌ ERROR: Version mismatch in native/src-tauri/tauri.conf.json (found "${tauriConf.version}", expected "${version}").`);
      process.exit(1);
    }
  }

  // Verify native/package.json version consistency
  const nativePkgPath = path.join(rootDir, 'native', 'package.json');
  if (fs.existsSync(nativePkgPath)) {
    const nativePkg = JSON.parse(fs.readFileSync(nativePkgPath, 'utf8'));
    if (nativePkg.version !== version) {
      console.error(`❌ ERROR: Version mismatch in native/package.json (found "${nativePkg.version}", expected "${version}").`);
      process.exit(1);
    }
  }

  // Verify root package.json version consistency
  const rootPkgPath = path.join(rootDir, 'package.json');
  if (fs.existsSync(rootPkgPath)) {
    const rootPkg = JSON.parse(fs.readFileSync(rootPkgPath, 'utf8'));
    if (rootPkg.version !== version) {
      console.error(`❌ ERROR: Version mismatch in root package.json (found "${rootPkg.version}", expected "${version}").`);
      process.exit(1);
    }
  }

  // Verify native/src-tauri/Cargo.toml version consistency
  const cargoTomlPath = path.join(rootDir, 'native', 'src-tauri', 'Cargo.toml');
  if (fs.existsSync(cargoTomlPath)) {
    const cargoToml = fs.readFileSync(cargoTomlPath, 'utf8');
    const match = cargoToml.match(/\[package\][\s\S]*?version\s*=\s*"([^"]+)"/);
    if (!match || match[1] !== version) {
      console.error(`❌ ERROR: Version mismatch in native/src-tauri/Cargo.toml (found "${match ? match[1] : 'unknown'}", expected "${version}").`);
      process.exit(1);
    }
  }

  console.log(`✅ Version & Changelog verified across all manifests: v${version}`);
}

function verifyReadme() {
  const readmePath = path.join(rootDir, 'README.md');
  if (!fs.existsSync(readmePath)) {
    console.error('❌ ERROR: README.md is missing.');
    process.exit(1);
  }

  const readmeContent = fs.readFileSync(readmePath, 'utf8');
  const lines = readmeContent.split('\n');

  // Remove code blocks prior to checking Markdown headers
  const contentWithoutCodeBlocks = readmeContent.replace(/```[\s\S]*?```/g, '');

  // Check 1: Single H1 title
  const h1Count = (contentWithoutCodeBlocks.match(/^#\s+/gm) || []).length;
  if (h1Count !== 1) {
    console.error(`❌ ERROR: README.md MUST have exactly one H1 title. Found ${h1Count}.`);
    process.exit(1);
  }

  // Check 2: Tagline formatted as blockquote
  const hasTaglineBlockquote = lines.some(line => line.trim().startsWith('>'));
  if (!hasTaglineBlockquote) {
    console.error('❌ ERROR: README.md MUST have a tagline formatted as a blockquote ("> ...") under the title per rules/readme.md.');
    process.exit(1);
  }

  // Check 3: Fenced code blocks language tags (opening fences must have a language specifier)
  let inCodeBlock = false;
  let missingLang = false;
  for (const line of lines) {
    const trimmed = line.trim();
    if (trimmed.startsWith('```')) {
      if (!inCodeBlock) {
        // Opening code block
        const lang = trimmed.slice(3).trim();
        if (!lang) {
          missingLang = true;
          break;
        }
        inCodeBlock = true;
      } else {
        // Closing code block
        inCodeBlock = false;
      }
    }
  }

  if (missingLang) {
    console.error('❌ ERROR: README.md has untagged code blocks. Every fenced code block MUST specify a language tag per rules/readme.md.');
    process.exit(1);
  }

  console.log('✅ README.md verified against rules/readme.md');
}

console.log('🔍 Running Relay Pre-commit / Pre-push Rule Verification...');
verifyVersionAndChangelog();
verifyReadme();
console.log('🎉 All repository rules verified successfully!');
