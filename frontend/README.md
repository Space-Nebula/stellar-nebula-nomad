# Frontend web-distribution configuration

Progressive Web App (PWA) configuration for the hosted web build of Stellar
Nebula Nomad — the same build the mobile shell (`mobile/`) wraps in a
WebView, distributed directly on the web as an installable app.

## Contents

| Path | Purpose |
| --- | --- |
| `manifest.webmanifest` | Web app manifest: identity, standalone display, theme colors, SVG icons (any + maskable). |
| `service-worker.js` | Offline support: precached app shell, network-first navigations with `offline.html` fallback, stale-while-revalidate static assets. RPC/API and cross-origin requests are never cached. |
| `offline.html` | Fallback page served when the network is down and nothing is cached. |
| `pwa/register-service-worker.js` | Registration helper with an update flow (`SKIP_WAITING` + controlled reload). |
| `pwa/install-prompt.js` | Captures `beforeinstallprompt` for a custom install button; reports iOS Safari manual-install and standalone states. |
| `icons/` | SVG app icons referenced by the manifest. |

## Integrating into the game build

Serve every file in this directory from the web root, then in the page:

```html
<link rel="manifest" href="/manifest.webmanifest">
<meta name="theme-color" content="#0b1020">
```

```js
import { registerServiceWorker } from "/pwa/register-service-worker.js";
import { createInstallPromptManager } from "/pwa/install-prompt.js";

registerServiceWorker({
  onUpdateAvailable(activate) {
    // e.g. show a "New version ready" toast whose click handler calls:
    activate();
  },
});

const installer = createInstallPromptManager({
  onCanInstall: () => installButton.hidden = false,
  onInstalled: () => installButton.hidden = true,
});
installButton.addEventListener("click", async () => {
  if ((await installer.promptInstall()) === "ios-manual") {
    showAddToHomeScreenInstructions();
  }
});
```

## Caching rules

- **Never cached:** non-GET requests, cross-origin requests, and same-origin
  paths under `/rpc` or `/api` — Soroban RPC responses are live chain state
  and must not be replayed from cache.
- **Precached (versioned):** app shell, manifest, icons, `offline.html`.
- **Runtime cached:** navigations (network-first) and static assets
  (stale-while-revalidate).

Bump `CACHE_VERSION` in `service-worker.js` when shipping a release that
must invalidate previously cached assets; old versioned caches are deleted
on activate.

## Notes

- The service worker requires a secure context (HTTPS or localhost).
- Inside the mobile WebView the worker is harmless where unsupported —
  registration is feature-detected and no-ops.
