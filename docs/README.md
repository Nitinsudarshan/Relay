# Relay documentation

The map of what's written down, where, and — importantly — what each file is
allowed to claim. Documentation that outlives its subject is worse than none,
because the next reader (human or agent) treats it as current.

## Living specification

These describe Relay as it is now. If one disagrees with the code, the code
is right and the document is a bug to be fixed in the same change.

| File | Scope |
|---|---|
| [product.md](product.md) | What Relay is, who it's for, and the differentiators. |
| [requirements.md](requirements.md) | Numbered functional and non-functional requirements. |
| [architecture.md](architecture.md) | The surfaces and how they relate. |
| [data-model.md](data-model.md) | Vault file layouts, frontmatter schemas, settings shape. |
| [api.md](api.md) | Tauri command conventions and the `CommandError` contract. |
| [user-flows.md](user-flows.md) | End-to-end flows through the shipped features. |
| [testing.md](testing.md) | What is tested, with what, and where the tests live. |
| [GOOGLE_OAUTH_SETUP.md](GOOGLE_OAUTH_SETUP.md) | The OAuth 2.0 PKCE architecture and Google Cloud setup. |

## Decision records

Append-only. Entries are never edited to match later reality — a decision
that was reversed gets a new entry saying so.

| File | Scope |
|---|---|
| [decisions.md](decisions.md) | The master decision log, numbered. |
| [decisions-push-to-talk-pill.md](decisions-push-to-talk-pill.md) | The pill redesign's own numbered decisions (PTT-00N). |

## Honest gaps

| File | Scope |
|---|---|
| [roadmap.md](roadmap.md) | What is real vs. stubbed, and what's next. |
| [../maybe_later.md](../maybe_later.md) | Deferred features, per `rules/maybe-later.md`. |

## Deep records

Long-form analyses written against a specific version. Each one states the
version it was traced from; read them as history, not as instructions.

| File | Scope |
|---|---|
| [meetings/MEETINGS_INTELLIGENCE_AUDIT.md](meetings/MEETINGS_INTELLIGENCE_AUDIT.md) | Stage-by-stage trace of the meetings pipeline (phase A audit). |
| [meetings/SUMMARY_QUALITY_REBUILD.md](meetings/SUMMARY_QUALITY_REBUILD.md) | Why v0.15.1 summaries were poor and what replaced the pipeline. |
| [meetings/MEETINGS_LEGACY_REMOVAL.md](meetings/MEETINGS_LEGACY_REMOVAL.md) | Archaeological record of the legacy meetings system (v0.1.0–v0.10.1). |

## Archive

Context preserved for removed features. Not specifications — nothing here
describes code that exists.

| File | Scope |
|---|---|
| [archive/prompt-mode.md](archive/prompt-mode.md) | Prompt Mode, removed in v0.15.0, and the thinking behind it. |

## Elsewhere in the repo

| Path | Scope |
|---|---|
| `../AGENTS.md` | Entry point for agents: surfaces, rule index, verification commands. |
| `../rules/` | Coding conventions, enforced per surface. |
| `../Meeting-rules/` | Behavioural specs for the meeting pipeline's prompts and extraction stages, cited directly from Rust doc comments. |
| `../CHANGELOG.md` | Per-version record of what shipped. |

## Adding a document

Before creating a new file, check whether the content belongs in one that
already exists. In order of preference:

1. A code comment, next to what it describes.
2. An entry in `decisions.md` (a choice) or `maybe_later.md` (a deferral).
3. An edit to one of the living-specification files above.
4. A new file — and then add it to this index in the same commit.

A per-task working document ("implementation plan", "update report", "audit
of what I'm about to do") does not belong in the repository at all. Its
durable content is a decision entry and a changelog line; the rest is
scaffolding that goes stale the moment the task ships.
