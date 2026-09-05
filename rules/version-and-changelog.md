---
trigger: always_on
description: Authoritative rules for release versioning, changelog ownership, and conventional change metadata.
---

# Versioning and Changelog Architecture

## Core Contract

> **DEVELOPMENT AGENTS DESCRIBE CHANGES.**  
> **THE RELEASE PIPELINE OWNS RELEASE VERSIONS.**

The repository has exactly one authoritative release-version owner: the GitHub Actions release pipeline (`.github/workflows/release.yml`).

- `VERSION` IS RELEASE METADATA, NOT DEVELOPMENT METADATA.
- `CHANGELOG.md` IS RELEASE HISTORY, NOT A PER-TASK WORK LOG.
- DEVELOPMENT AGENTS MUST NOT BUMP `VERSION`.
- DEVELOPMENT AGENTS MUST NOT CREATE RELEASE CHANGELOG ENTRIES.
- THE RELEASE PIPELINE IS THE SOLE AUTHORITY FOR VERSION ASSIGNMENT.

---

## 1. No Agent-Owned Version Bumping

During normal development tasks, agents and human contributors **must not**:
- Increment `VERSION`
- Calculate the next production release version
- Modify the canonical release version
- Add release headings (e.g. `## [x.y.z]`) to `CHANGELOG.md`
- Create release tags or GitHub Releases

Never independently bump `VERSION` as part of a feature, bug fix, or refactoring task.

---

## 2. Canonical Version File (`VERSION`)

The root-level `VERSION` file remains the canonical source of truth for Relay's **latest released version**.

- It does **not** represent the version of an individual development task or pull request.
- If `VERSION` is `0.41.0`, it remains `0.41.0` while 10, 20, or 50 PRs are developed, merged, and integrated into `main`.
- Only the release workflow updates `VERSION` when publishing a release (e.g. `0.41.0` → `0.42.0`).

---

## 3. Atomic Manifest Synchronization

Relay maintains version consistency across five synchronized manifests:

1. `VERSION` (canonical plain-text version at repo root)
2. `package.json` (root monorepo manifest)
3. `native/package.json` (desktop frontend package)
4. `native/src-tauri/tauri.conf.json` (Tauri application configuration)
5. `native/src-tauri/Cargo.toml` (Rust backend crate)

**Ownership**:
- All five files must stay strictly identical in version.
- **The release pipeline synchronizes all five locations atomically.**
- Normal development commits must never introduce version churn or mismatches across these files.

---

## 4. Changelog Ownership (`CHANGELOG.md`)

`CHANGELOG.md` is the canonical historical release changelog.

- It documents **published releases**, not ongoing task transcripts.
- Development agents must not prepend changelog entries to `CHANGELOG.md` for individual tasks or PRs.
- The release pipeline owns:
  1. Collecting changes merged since the prior release
  2. Categorizing entries
  3. Determining the release version and release date
  4. Prepending the formatted release entry to `CHANGELOG.md`
  5. Publishing the release notes to GitHub Releases

---

## 5. Change Metadata via Conventional Commits

Development agents communicate what changed and its intended impact via **Conventional Commits** in commit messages and PR descriptions.

### Commit Format

```text
<type>(<scope>): <short summary>

[optional body with details and surface impact]

[optional footer(s), e.g. BREAKING CHANGE: description]
```

### Types & Release Impact Mapping

| Type | Release Impact | Purpose & Categories |
|---|---|---|
| `fix` | **patch** | Bug fixes and runtime corrections (→ `Fixes`) |
| `feat` | **minor** | New features, capabilities, or user-facing modules (→ `Features`) |
| `refactor` | **patch** (or none) | Internal code restructuring with no behavior change (→ `Improvements`) |
| `perf` | **patch** | Performance improvements (→ `Improvements`) |
| `sec` | **patch** | Security hardening or vulnerability fixes (→ `Security`) |
| `docs` | none | Documentation updates only |
| `test` | none | Adding or updating tests only |
| `chore` | none | Maintenance, dependencies, or tooling |
| `BREAKING CHANGE:` | **major** | Incompatible API, schema, or system behavioral changes (→ `Breaking Changes`) |

### Surface Tagging

Where applicable, note the affected Relay surface in the scope or description:
- `native` (desktop React frontend)
- `tauri` or `backend` (Rust backend)
- `web` (Next.js dashboard)
- `meetings` (meetings pipeline)
- `retrieval` (vault / vector search)
- `capture` (audio / dictation / screen capture)

Examples:
- `feat(meetings): add Google Calendar event auto-linking`
- `fix(backend): prevent duplicate transcript chunk buffering`
- `refactor(vault): simplify note retrieval pipeline`
- `docs(architecture): clarify meeting reminder state machine`

---

## 6. Multi-Agent Concurrent Safety

Relay is developed concurrently across multiple tools and agents (local IDE, Antigravity, Cloud Code, ChatGPT).

Under this contract:
1. **Agent A** and **Agent B** branch from `main` at `VERSION = 0.41.0`.
2. Agent A implements feature X; Agent B fixes bug Y.
3. Both agents run verification gates (`npm run verify:rules`, tests, linters).
4. Neither agent modifies `VERSION` or `CHANGELOG.md`.
5. Both branches merge cleanly without version conflict or changelog merge collisions.
6. When ready, the release pipeline runs once, evaluates all merged changes, bumps `VERSION` to `0.42.0`, writes the comprehensive changelog, and tags the release.

---

## 7. Traceability

Every release must be fully traceable back to its source:
```
Release Version (e.g. 0.42.0)
    ↓
Git Tag (e.g. v0.42.0)
    ↓
Release Commit (chore(release): v0.42.0)
    ↓
Merged Pull Requests
    ↓
Conventional Commits
    ↓
Actual Code Changes (git diff)
```

Every changelog item generated by the release pipeline derives directly from merged PRs and commit messages. Entries are never fabricated.

---

## 8. Verification Commands

- **Development rule check** (runs on pre-commit, pre-push, and CI):
  ```bash
  npm run verify:rules
  ```
  Verifies that `VERSION` is valid semver, all 5 manifests agree, `CHANGELOG.md` exists, and `README.md` passes structural checks. Ensures development tasks haven't introduced version divergence.

- **Release rule check** (runs in release workflow):
  ```bash
  npm run verify:release
  ```
  Verifies that `VERSION` agrees across all 5 manifests AND that `CHANGELOG.md` contains the topmost release entry matching `VERSION`.
