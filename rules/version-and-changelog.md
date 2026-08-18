---
trigger: always_on
description: Rules for maintaining application versioning and changelog registry after every successful run/task before completion or push.
---

# Versioning and Changelog Maintenance

All development tasks on Relay must maintain a root-level `VERSION` file and
`CHANGELOG.md`. NGConnect's equivalent lived in a single `src/lib/version-config.ts`
because it's a single-language Next.js app; Relay is polyglot (Rust +
TypeScript across two frontends), so the version registry lives at the repo
root instead of inside any one surface's source tree.

## Mandatory Requirement

After every successful task execution (and before any push/commit), you
**must**:

1. Read `VERSION` and `CHANGELOG.md` and inspect the latest entry.
2. Determine the appropriate version increment based on the work completed:
   - **Patch (`x.xx.xx + 1`)**: Bug fixes, minor improvements, refactoring,
     patch features.
   - **Minor (`x.xx+1.00`)**: New module additions, major features, schema
     additions (in either the Rust backend or either frontend).
   - **Major (`x+1.00.00`)**: System overhauls, major releases.
3. Update `VERSION`.
4. Prepend a new entry to `CHANGELOG.md`:
   - `version`: string matching `VERSION`
   - `date`: YYYY-MM-DD
   - `title`: short descriptive title of the update
   - `type`: `patch` | `minor` | `major`
   - `changes`: bullet items with a `category` (`Features` | `Improvements`
     | `Fixes` | `Security`) and a concise description — note which surface
     (`native/`, `web/`, or both) each change touched, since this is a
     multi-surface repo.
5. **Verification**: check every change-item description against the
   actual `git diff` (or file changes) of the commit/task it describes
   before finalizing the entry.
