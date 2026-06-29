/**
 * WalletConnect v2 integration for Stellar Nebula Nomad.
 *
 * WalletConnect provides a universal wallet connection protocol
 * supporting mobile wallets and desktop applications.
 */

export interface WalletConnectConfig {
  /** WalletConnect project ID (from https://cloud.walletconnect.com) */
  projectId: string;
  /** Stellar network passphrase */
  networkPassphrase?: string;
  /** Optional relay URL override */
  relayUrl?: string;
  /** App metadata */
  metadata?: {
    name: string;
    description: string;
    url: string;
    icons: string[];
  };
}

export interface WCSession {
  /** Session topic */
  topic: string;
  /** Connected public key */
  publicKey: string;
  /** Session expiry (unix timestamp) */
  expiry: number;
}

const STELLAR_NAMESPACE = "stellar";
const STELLAR_METHODS = [
  "stellar_signAndSubmitXDR",
  "stellar_signXDR",
];
const STELLAR_EVENTS = ["accountsChanged", "chainChanged"];
const STELLAR_TESTNET_CHAIN = "stellar:testnet";
const STELLAR_MAINNET_CHAIN = "stellar:pubnet";

/**
 * WalletConnect v2 client for Stellar.
 *
 * This is a high-level wrapper that manages sessions and signing.
 * Requires @walletconnect/sign-client as a peer dependency.
 *
 * Usage:
 *   const wc = new WalletConnectClient({ projectId: "..." });
 *   const session = await wc.connect();
 *   const signedXdr = await wc.signTransaction(xdr, passphrase);
 */
export class WalletConnectClient {
  private projectId: string;
  private networkPassphrase: string;
  private relayUrl: string;
  private metadata: WalletConnectConfig["metadata"];
  private session: WCSession | null = null;

  constructor(config: WalletConnectConfig) {
    this.projectId = config.projectId;
    this.networkPassphrase =
      config.networkPassphrase ?? "Test SDF Network ; September 2015";
    this.relayUrl = config.relayUrl ?? "wss://relay.walletconnect.com";
    this.metadata = config.metadata ?? {
      name: "Nebula Nomad",
      description: "Decentralized space exploration game on Stellar",
      url: "https://nebulanomad.io",
      icons: ["https://nebulanomad.io/icon.png"],
    };
  }

  /**
   * Initialize WalletConnect and open the connection modal.
   * Returns the connected session with the user's public key.
   */
  async connect(): Promise<WCSession> {
    const { SignClient } = await this.loadSignClient();

    const client = await SignClient.init({
      projectId: this.projectId,
      relayUrl: this.relayUrl,
      metadata: this.metadata,
    });

    const chain = this.networkPassphrase.includes("Test")
      ? STELLAR_TESTNET_CHAIN
      : STELLAR_MAINNET_CHAIN;

    const { uri, approval } = await client.connect({
      requiredNamespaces: {
        [STELLAR_NAMESPACE]: {
          methods: STELLAR_METHODS,
          chains: [chain],
          events: STELLAR_EVENTS,
        },
      },
    });

    // If URI is available, show QR code modal
    if (uri) {
      this.openModal(uri);
    }

    const session = await approval();

    const accounts = session.namespaces[STELLAR_NAMESPACE]?.accounts ?? [];
    const publicKey = accounts[0]?.split(":")[2] ?? "";

    if (!publicKey) {
      throw new Error("No Stellar account found in WalletConnect session.");
    }

    this.session = {
      topic: session.topic,
      publicKey,
      expiry: session.expiry,
    };

    this.closeModal();

    return this.session;
  }

  /**
   * Disconnect the current session.
   */
  async disconnect(): Promise<void> {
    if (!this.session) return;

    const { SignClient } = await this.loadSignClient();
    const client = await SignClient.init({
      projectId: this.projectId,
      relayUrl: this.relayUrl,
    });

    await client.disconnect({
      topic: this.session.topic,
      reason: { code: 6000, message: "User disconnected" },
    });

    this.session = null;
  }

  /**
   * Sign a transaction via WalletConnect.
   */
  async signTransaction(
    xdr: string,
    networkPassphrase: string,
  ): Promise<string> {
    if (!this.session) {
      throw new Error("Not connected. Call connect() first.");
    }

    const { SignClient } = await this.loadSignClient();
    const client = await SignClient.init({
      projectId: this.projectId,
      relayUrl: this.relayUrl,
    });

    const chain = networkPassphrase.includes("Test")
      ? STELLAR_TESTNET_CHAIN
      : STELLAR_MAINNET_CHAIN;

    const result = await client.request({
      topic: this.session.topic,
      chainId: chain,
      request: {
        method: "stellar_signXDR",
        params: { xdr, networkPassphrase },
      },
    });

    return (result as { signedXDR: string }).signedXDR;
  }

  /**
   * Sign and submit a transaction via WalletConnect.
   */
  async signAndSubmitTransaction(
    xdr: string,
    networkPassphrase: string,
  ): Promise<{ signedXDR: string; txHash: string }> {
    if (!this.session) {
      throw new Error("Not connected. Call connect() first.");
    }

    const { SignClient } = await this.loadSignClient();
    const client = await SignClient.init({
      projectId: this.projectId,
      relayUrl: this.relayUrl,
    });

    const chain = networkPassphrase.includes("Test")
      ? STELLAR_TESTNET_CHAIN
      : STELLAR_MAINNET_CHAIN;

    const result = await client.request({
      topic: this.session.topic,
      chainId: chain,
      request: {
        method: "stellar_signAndSubmitXDR",
        params: { xdr, networkPassphrase },
      },
    });

    return result as { signedXDR: string; txHash: string };
  }

  /**
   * Get the current session.
   */
  getSession(): WCSession | null {
    return this.session;
  }

  /**
   * Check if currently connected.
   */
  isConnected(): boolean {
    return this.session !== null && this.session.expiry > Date.now() / 1000;
  }

  /**
   * Dynamically load the WalletConnect SignClient.
   * This allows the module to work without bundling the SDK upfront.
   */
  private async loadSignClient() {
    try {
      const mod = await import("@walletconnect/sign-client");
      return { SignClient: mod.default ?? mod.SignClient };
    } catch {
      throw new Error(
        "WalletConnect SignClient not found. Install @walletconnect/sign-client.",
      );
    }
  }

  /**
   * Open a modal displaying the WalletConnect URI (QR code).
   * Uses a simple overlay -- replace with your preferred QR modal library.
   */
  private openModal(uri: string): void {
    if (typeof document === "undefined") return;

    const overlay = document.createElement("div");
    overlay.id = "wc-modal";
    overlay.style.cssText =
      "position:fixed;inset:0;background:rgba(0,0,0,0.7);display:flex;align-items:center;justify-content:center;z-index:9999";

    const box = document.createElement("div");
    box.style.cssText =
      "background:#1a1a2e;border-radius:12px;padding:24px;max-width:400px;text-align:center;color:#fff";

    box.innerHTML = `
      <h3 style="margin:0 0 16px">Connect Wallet</h3>
      <p style="font-size:14px;opacity:0.7;margin:0 0 16px">Scan this code with your Stellar wallet</p>
      <pre style="word-break:break-all;font-size:11px;background:#0d0d1a;padding:12px;border-radius:8px;text-align:left;max-height:200px;overflow:auto">${uri}</pre>
      <button id="wc-cancel" style="margin-top:16px;padding:8px 24px;border:none;border-radius:6px;background:#e74c3c;color:#fff;cursor:pointer">Cancel</button>
    `;

    overlay.appendChild(box);
    document.body.appendChild(overlay);

    document.getElementById("wc-cancel")?.addEventListener("click", () => {
      this.closeModal();
    });
  }

  /**
   * Close the WalletConnect modal.
   */
  private closeModal(): void {
    if (typeof document === "undefined") return;
    document.getElementById("wc-modal")?.remove();
  }
}
