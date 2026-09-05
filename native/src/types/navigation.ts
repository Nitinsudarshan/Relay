/**
 * The native window's navigation vocabulary.
 *
 * One definition, imported by everything that navigates: `App`'s router, the
 * sidebar, Home's cards and every page that hands the user to another surface.
 * It lives here rather than in `App.tsx` so a page can be typed against it
 * without importing the component that renders it — the `tab as MainTabType`
 * casts that used to sit on those call sites were exactly what let
 * `onNavigateTab('scribbles')` reach a tab that does not exist.
 */
export type MainTabType =
  | 'home'
  | 'capture'
  | 'meetings'
  | 'scribble'
  | 'graph'
  | 'files'
  | 'captures'
  | 'talkback'
  | 'diagnostics'
  | 'settings';
