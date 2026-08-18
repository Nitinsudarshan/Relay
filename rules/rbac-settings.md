---
description: Why a granular RBAC system is intentionally not built for Relay yet
---

# RBAC — Not Built, and Why

NGConnect's `rbac-settings.md` described a granular, resource-based RBAC
system (role/team/user-level overrides, a permission-resource registry, a
management UI). **Relay does not have this, and shouldn't get it yet.**

## Why this is deliberately absent

- `Relay - Decision Log.md`'s primary-users framing (decision-log context)
  is the builder themselves, personal use — a single-user product has
  nothing for RBAC to gate.
- A team/enterprise, mutual-sharing model was raised as a real future
  direction (see `Relay - Decision Log.md`'s "Noted for later, not decided"
  section) but is explicitly **not decided or scoped** — building
  permission infrastructure now would be exactly the kind of "design for
  hypothetical future requirements" the build prompt's operating mode
  argues against.

## If the team/sharing direction is ever picked up

Decision 12's hybrid-mode auth foundation (real login against a cloud
backend) is a reasonable base to extend — accounts already exist once
that's built, which is most of what a future permission system would need
to attach to. Revisit this file, and likely reintroduce something closer to
NGConnect's actual RBAC shape, only once that direction becomes a real,
scoped decision — not before.
