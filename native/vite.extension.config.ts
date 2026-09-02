/// <reference types="node" />
import { defineConfig } from 'vite';
import path from 'path';

/**
 * Builds the Relay browser extension's three entry points.
 *
 * Three separate builds rather than one: the content bundle must be a
 * self-contained classic script (it is injected with
 * `chrome.scripting.executeScript({ files })`, which does not run modules),
 * and Rollup cannot emit several IIFE bundles from one build. Each build
 * therefore names its target through `--mode`:
 *
 *   vite build -c vite.extension.config.ts --mode content
 *   vite build -c vite.extension.config.ts --mode background
 *   vite build -c vite.extension.config.ts --mode options
 *
 * `npm run build:extension` runs all three.
 *
 * The sources live under `src/webcapture/` so they are typechecked by
 * `npm run typecheck` and unit-tested by `npm test` alongside the rest of the
 * app, instead of sitting in a parallel tree with its own toolchain that
 * nothing in CI would ever look at.
 */
const TARGETS = {
  content: { entry: 'src/webcapture/content.ts', fileName: 'relay-extract', format: 'iife' },
  background: { entry: 'src/webcapture/background.ts', fileName: 'background', format: 'es' },
  options: { entry: 'src/webcapture/options.ts', fileName: 'options', format: 'es' },
} as const;

export default defineConfig(({ mode }) => {
  const target = TARGETS[mode as keyof typeof TARGETS];
  if (!target) {
    throw new Error(
      `Unknown extension build target "${mode}". Expected one of: ${Object.keys(TARGETS).join(', ')}`,
    );
  }

  return {
    build: {
      // Each target writes one file into the extension directory and must not
      // wipe the manifest, the options page, or its sibling bundles.
      outDir: 'browser-extension',
      emptyOutDir: false,
      target: 'es2022',
      minify: false,
      lib: {
        entry: path.resolve(__dirname, target.entry),
        formats: [target.format],
        fileName: () => `${target.fileName}.js`,
        name: 'RelayCapture',
      },
    },
  };
});
