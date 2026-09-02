# Relay Capture — browser extension

Reads the page you are looking at and posts it to Relay on your own machine.

## Build

```bash
cd native
npm install
npm run build:extension
```

That writes `background.js`, `relay-extract.js`, and `options.js` next to this
file. The build output is not committed; `manifest.json`, `options.html`, and
this README are.

## Install (Chrome or Edge)

1. Open `chrome://extensions` (`edge://extensions`) and turn on **Developer mode**.
2. **Load unpacked**, and choose this `browser-extension` directory.
3. In Relay: **Settings → Capture** → turn **Browser capture** on, and copy the
   port and pairing token.
4. Open the extension's **Options** and paste both in, then **Save and test**.

## Capture a page

Press **Ctrl+Shift+Y** (Command+Shift+Y on macOS) or click the Relay toolbar
button. Change the shortcut at `chrome://extensions/shortcuts`.

The toolbar badge reports what happened: `…` capturing, `↑` sending, `✓` saved,
`✕` failed — hover it for the reason.

## What it is allowed to do

| Permission | Why |
|---|---|
| `activeTab` | Read the page — but only the one tab, and only when you press the shortcut or the button. The grant ends when you navigate away. |
| `scripting` | Inject the extraction code into that tab on demand. |
| `storage` | Remember Relay's port and your pairing token, locally. |
| `http://127.0.0.1/*` | Send the capture to Relay on this computer. |

There is no `<all_urls>`, no declared content script, and no permission to any
website. A page the extension has not been explicitly invoked on is never read.
