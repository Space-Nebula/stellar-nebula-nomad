/**
 * Service worker registration with an update flow.
 *
 * Usage from the game frontend:
 *
 *   import { registerServiceWorker } from "./pwa/register-service-worker.js";
 *
 *   registerServiceWorker({
 *     onUpdateAvailable(activate) {
 *       // Show a "New version available" toast; call activate() on click.
 *       activate();
 *     },
 *   });
 *
 * When a new worker is waiting, `onUpdateAvailable` receives an `activate`
 * callback that messages the worker to skip waiting; once it takes control
 * the page reloads so all assets come from the new cache version.
 */

/**
 * @param {Object} [options]
 * @param {string} [options.url] Service worker script URL.
 * @param {string} [options.scope] Registration scope.
 * @param {(activate: () => void) => void} [options.onUpdateAvailable]
 *   Invoked when an updated worker is installed and waiting.
 * @param {(error: unknown) => void} [options.onError]
 * @returns {Promise<ServiceWorkerRegistration | null>}
 */
export async function registerServiceWorker(options = {}) {
  const {
    url = "/service-worker.js",
    scope = "/",
    onUpdateAvailable,
    onError,
  } = options;

  if (!("serviceWorker" in navigator)) return null;

  try {
    const registration = await navigator.serviceWorker.register(url, { scope });

    const notifyIfWaiting = () => {
      // Only prompt when an active worker exists — a first install has
      // nothing to "update" and should activate silently.
      if (
        registration.waiting &&
        navigator.serviceWorker.controller &&
        onUpdateAvailable
      ) {
        const waiting = registration.waiting;
        onUpdateAvailable(() => {
          waiting.postMessage({ type: "SKIP_WAITING" });
        });
      }
    };

    notifyIfWaiting();
    registration.addEventListener("updatefound", () => {
      const installing = registration.installing;
      if (!installing) return;
      installing.addEventListener("statechange", notifyIfWaiting);
    });

    // Reload once the new worker takes over so the page and its assets
    // are consistent with the new cache version. Guarded so a first
    // install (no previous controller) doesn't trigger a reload loop.
    let hadController = Boolean(navigator.serviceWorker.controller);
    navigator.serviceWorker.addEventListener("controllerchange", () => {
      if (hadController) {
        window.location.reload();
      }
      hadController = true;
    });

    return registration;
  } catch (error) {
    if (onError) onError(error);
    else console.warn("[pwa] service worker registration failed:", error);
    return null;
  }
}
