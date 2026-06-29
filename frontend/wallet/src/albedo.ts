/**
 * Albedo wallet integration for Stellar Nebula Nomad.
 *
 * Albedo is a popular Stellar wallet that provides a browser-based
 * signing experience without requiring browser extensions.
 */

export interface AlbedoConfig {
  /** Albedo API endpoint (default: https://albedo.link) */
  apiUrl?: string;
}

export interface AlbedoSignResult {
  /** Signed transaction XDR */
  signed_envelope_xdr: string;
  /** Whether the transaction was signed */
  signed: boolean;
  /** Network passphrase used */
  network: string;
}

export interface AlbedoPublicKeyResult {
  /** User's Stellar public key */
  pubkey: string;
  /** Whether the user approved the connection */
  approved: boolean;
  /** Signed challenge for verification */
  signed_message: string;
  /** Signature of the challenge */
  signature: string;
}

/**
 * Albedo intent-based API client.
 *
 * Uses window.open to trigger Albedo's authorization flow.
 * No SDK dependency required -- works via postMessage.
 */
export class AlbedoClient {
  private apiUrl: string;

  constructor(config?: AlbedoConfig) {
    this.apiUrl = config?.apiUrl ?? "https://albedo.link";
  }

  /**
   * Request the user's public key via Albedo.
   */
  async getPublicKey(): Promise<AlbedoPublicKeyResult> {
    return this.requestIntent("pubkey", {});
  }

  /**
   * Request a transaction signature via Albedo.
   */
  async signTransaction(
    xdr: string,
    networkPassphrase: string,
    opts?: {
      memo?: string;
      callback?: string;
    },
  ): Promise<AlbedoSignResult> {
    return this.requestIntent("tx", {
      xdr,
      network: networkPassphrase,
      ...(opts?.memo && { memo: opts.memo }),
      ...(opts?.callback && { callback: opts.callback }),
    });
  }

  /**
   * Check if Albedo is available (always true -- it's web-based).
   */
  isAvailable(): boolean {
    return typeof window !== "undefined";
  }

  /**
   * Generic intent request handler.
   * Opens Albedo in a popup and waits for the response via postMessage.
   */
  private async requestIntent<T>(intent: string, params: Record<string, string>): Promise<T> {
    return new Promise((resolve, reject) => {
      const popup = window.open(
        `${this.apiUrl}/#/${intent}?${new URLSearchParams(params).toString()}`,
        "albedo",
        "width=400,height=600,menubar=no,toolbar=no",
      );

      if (!popup) {
        reject(new Error("Failed to open Albedo popup. Please allow popups for this site."));
        return;
      }

      const handler = (event: MessageEvent) => {
        if (event.origin !== this.apiUrl) return;
        window.removeEventListener("message", handler);
        popup.close();

        if (event.data?.error) {
          reject(new Error(event.data.error));
        } else {
          resolve(event.data as T);
        }
      };

      window.addEventListener("message", handler);

      // Timeout after 5 minutes
      setTimeout(() => {
        window.removeEventListener("message", handler);
        popup.close();
        reject(new Error("Albedo authorization timed out."));
      }, 300_000);
    });
  }
}
