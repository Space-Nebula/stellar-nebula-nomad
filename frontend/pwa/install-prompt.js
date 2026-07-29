/**
 * Install-prompt manager for the web distribution.
 *
 * Chromium browsers gate PWA installation behind the `beforeinstallprompt`
 * event: it must be captured, suppressed, and replayed from a user gesture.
 * iOS Safari has no programmatic install at all — users add to the home
 * screen manually — so this module also reports when to show manual
 * instructions instead of an install button.
 *
 * Usage:
 *
 *   import { createInstallPromptManager } from "./pwa/install-prompt.js";
 *
 *   const installer = createInstallPromptManager({
 *     onCanInstall() { showInstallButton(); },
 *     onInstalled() { hideInstallButton(); },
 *   });
 *
 *   installButton.addEventListener("click", async () => {
 *     const outcome = await installer.promptInstall();
 *     if (outcome === "ios-manual") showIosInstructions();
 *   });
 */

/**
 * @param {Object} [options]
 * @param {() => void} [options.onCanInstall] Native install prompt captured.
 * @param {() => void} [options.onInstalled] App was installed.
 * @param {Window} [options.win] Injectable window for testing.
 * @returns {{
 *   promptInstall: () => Promise<"accepted" | "dismissed" | "ios-manual" | "unavailable">,
 *   canInstall: () => boolean,
 *   isStandalone: () => boolean,
 *   isIosSafari: () => boolean,
 *   dispose: () => void,
 * }}
 */
export function createInstallPromptManager(options = {}) {
  const { onCanInstall, onInstalled, win = window } = options;

  /** @type {any} Deferred BeforeInstallPromptEvent, if captured. */
  let deferredPrompt = null;

  const isStandalone = () =>
    (win.matchMedia && win.matchMedia("(display-mode: standalone)").matches) ||
    win.navigator.standalone === true;

  const isIosSafari = () =>
    /iphone|ipad|ipod/i.test(win.navigator.userAgent) &&
    !/crios|fxios/i.test(win.navigator.userAgent);

  const handleBeforeInstallPrompt = (event) => {
    // Suppress the mini-infobar; replay later from promptInstall().
    event.preventDefault();
    deferredPrompt = event;
    if (!isStandalone() && onCanInstall) onCanInstall();
  };

  const handleAppInstalled = () => {
    deferredPrompt = null;
    if (onInstalled) onInstalled();
  };

  win.addEventListener("beforeinstallprompt", handleBeforeInstallPrompt);
  win.addEventListener("appinstalled", handleAppInstalled);

  return {
    /** True when the captured native prompt can be replayed right now. */
    canInstall: () => deferredPrompt !== null && !isStandalone(),

    isStandalone,
    isIosSafari,

    /**
     * Shows the native install prompt if one was captured. On iOS Safari
     * (no native prompt) resolves "ios-manual" so the caller can render
     * add-to-home-screen instructions.
     */
    async promptInstall() {
      if (isStandalone()) return "unavailable";
      if (deferredPrompt) {
        const prompt = deferredPrompt;
        deferredPrompt = null;
        prompt.prompt();
        const choice = await prompt.userChoice;
        return choice && choice.outcome === "accepted"
          ? "accepted"
          : "dismissed";
      }
      if (isIosSafari()) return "ios-manual";
      return "unavailable";
    },

    dispose() {
      win.removeEventListener("beforeinstallprompt", handleBeforeInstallPrompt);
      win.removeEventListener("appinstalled", handleAppInstalled);
      deferredPrompt = null;
    },
  };
}
