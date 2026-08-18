# Relay — Testing Strategy

Per `rules/testing.md`, every domain module and surface has dedicated test coverage.

## 1. Rust Backend Unit & Integration Tests (`native/src-tauri/`)

- Run tests using:
  ```powershell
  cd native/src-tauri
  cargo test
  cargo clippy
  ```

- Coverage targets:
  - `providers`: Test Ollama response parser and fallback error mapping.
  - `pipeline`: Test JSON parsing of LLM meeting outputs and Kanban card extraction.
  - `triggers`: Test phrase matcher, fuzzy string comparison, and parameter extraction.
  - `vault`: Test Markdown frontmatter parsing and note file CRUD operations.

## 2. React Native Desktop UI Tests (`native/src/`)

- Framework: Vitest + React Testing Library.
- Run tests using:
  ```powershell
  cd native
  npm run test
  ```
- Coverage targets:
  - PTTWidget rendering, recording state toggle, hotkey events.
  - Kanban board column rendering and card filtering.
  - Trigger settings form validation.

## 3. Web Dashboard Tests (`web/`)

- Framework: Vitest / Playwright.
- Run tests using:
  ```powershell
  cd web
  npm run test
  npm run build
  ```
- Coverage targets:
  - Supabase client initialization.
  - Route handler response format (`{ success, data, error }`).
