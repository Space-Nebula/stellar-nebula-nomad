/**
 * Service worker for the Stellar Nebula Nomad web distribution.
 *
 * Caching strategy:
 *  - App shell + offline assets are precached at install and served
 *    cache-first (they're versioned by CACHE_VERSION, so stale entries are
 *    dropped wholesale on activate).
 *  - Navigations are network-first with a cached-shell fallback, ending at
 *    offline.html when the network is down and nothing is cached.
 *  - Same-origin static assets (scripts, styles, images, fonts, wasm) use
 *    stale-while-revalidate: served from cache immediately, refreshed in
 *    the background.
 *  - Everything else — Soroban RPC calls, WebSocket upgrades, cross-origin
 *    requests, non-GET methods — is never cached: chain state must always
 *    be live, never replayed from cache.
 */

const CACHE_VERSION = "v1";
const PRECACHE = `nebula-nomad-precache-${CACHE_VERSION}`;
const RUNTIME = `nebula-nomad-runtime-${CACHE_VERSION}`;

/** Core assets required for the app shell to boot offline. */
const OFFLINE_ASSETS = [
  "/",
  "/offline.html",
  "/manifest.webmanifest",
  "/icons/icon.svg",
  "/icons/icon-maskable.svg",
];

const STATIC_DESTINATIONS = new Set([
  "script",
  "style",
  "image",
  "font",
  "worker",
]);

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(PRECACHE).then((cache) => cache.addAll(OFFLINE_ASSETS)),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(
          keys
            .filter(
              (key) =>
                key.startsWith("nebula-nomad-") &&
                key !== PRECACHE &&
                key !== RUNTIME,
            )
            .map((key) => caches.delete(key)),
        ),
      )
      .then(() => self.clients.claim()),
  );
});

// The page can promote a waiting update immediately (see
// pwa/register-service-worker.js update flow).
self.addEventListener("message", (event) => {
  if (event.data && event.data.type === "SKIP_WAITING") {
    self.skipWaiting();
  }
});

self.addEventListener("fetch", (event) => {
  const request = event.request;
  if (request.method !== "GET") return;

  const url = new URL(request.url);
  if (url.origin !== self.location.origin) return;

  // Live chain data must never be served from cache.
  if (url.pathname.startsWith("/rpc") || url.pathname.startsWith("/api")) {
    return;
  }

  if (request.mode === "navigate") {
    event.respondWith(networkFirstNavigation(request));
    return;
  }

  if (STATIC_DESTINATIONS.has(request.destination)) {
    event.respondWith(staleWhileRevalidate(request));
    return;
  }

  event.respondWith(cacheFirst(request));
});

async function networkFirstNavigation(request) {
  try {
    const response = await fetch(request);
    if (response.ok) {
      const cache = await caches.open(RUNTIME);
      cache.put(request, response.clone());
    }
    return response;
  } catch {
    const cached = await caches.match(request);
    if (cached) return cached;
    const shell = await caches.match("/");
    if (shell) return shell;
    return caches.match("/offline.html");
  }
}

async function staleWhileRevalidate(request) {
  const cache = await caches.open(RUNTIME);
  const cached = await cache.match(request);
  const refresh = fetch(request)
    .then((response) => {
      if (response.ok) cache.put(request, response.clone());
      return response;
    })
    .catch(() => undefined);
  return cached || refresh.then((r) => r || fetchOfflineFallback(request));
}

async function cacheFirst(request) {
  const cached = await caches.match(request);
  if (cached) return cached;
  try {
    const response = await fetch(request);
    if (response.ok) {
      const cache = await caches.open(RUNTIME);
      cache.put(request, response.clone());
    }
    return response;
  } catch {
    return fetchOfflineFallback(request);
  }
}

async function fetchOfflineFallback(request) {
  if (request.destination === "document") {
    const offline = await caches.match("/offline.html");
    if (offline) return offline;
  }
  return Response.error();
}
