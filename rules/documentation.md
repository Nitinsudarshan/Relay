---
trigger: always_on
description: Comment and documentation format for generated code
globs: "native/src/**/*.ts, native/src/**/*.tsx, native/src-tauri/**/*.rs, web/src/**/*.ts, web/src/**/*.tsx"
---

# Code Documentation Rules

All generated code must include concise comments explaining intent, not
restating what the code obviously does. Applies across all three surfaces —
TypeScript conventions below for `native/src/`/`web/src/`, Rust doc-comment
equivalent for `native/src-tauri/`.

## TypeScript/React format

- Exported components, hooks, and functions get a JSDoc block above the
  declaration:

  ```ts
  /**
   * Classifies a transcript against the user's configured trigger phrases.
   * @param transcript - Raw STT output for the current capture
   */
  export const useTriggerMatch = (transcript: string) => { ... }
  ```

- Document: purpose of the component/function, non-obvious prop behavior
  (only ones whose meaning isn't clear from the name/type), and any
  non-obvious logic (a tricky LanceDB query assumption, a workaround for a
  library quirk, a magic number).
- Use inline `//` comments only for logic that isn't self-explanatory from
  variable/function names.
- Do not add a comment that just repeats the code.
- No need to document internal, non-exported helper functions unless the
  logic is genuinely non-obvious.

## Rust format

- Exported (`pub`) functions, structs, and modules get a `///` doc comment
  above the declaration, same bar as JSDoc above: purpose, non-obvious
  parameter behavior, non-obvious logic.
- Use inline `//` comments only for logic that isn't self-explanatory.
- Don't document a private helper unless its logic is genuinely non-obvious.
