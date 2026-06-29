/**
 * Unified wallet provider for Stellar Nebula Nomad.
 *
 * Supports multiple wallet types:
 * - Albedo (web-based, no extension required)
 * - WalletConnect v2 (mobile and desktop wallets)
 * - Freighter (browser extension)
 *
 * Usage:
 *   const provider = new WalletProvider();
 *   await provider.connect("albedo");
 *   const signed = await provider.signTransaction(xdr, passphrase);
 */

import { AlbedoClient, type AlbedoSignResult } from "./albedo";
import {
  WalletConnectClient,
  type WalletConnectConfig,
  type WCSession,
} from "./walletconnect";

export type WalletType = "albedo" | "walletconnect" | "freighter";

export interface WalletConnection {
  type: WalletType;
  publicKey: string;
}

export interface WalletProviderConfig {
  /** WalletConnect project ID (required for WalletConnect) */
  walletConnectProjectId?: string;
  /** Stellar network passphrase */
  networkPassphrase?: string;
}

/**
 * Unified wallet provider that supports Albedo, WalletConnect, and Freighter.
 */
export class WalletProvider {
  private albedo: AlbedoClient | null = null;
  private walletConnect: WalletConnectClient | null = null;
  private connection: WalletConnection | null = null;
  private networkPassphrase: string;

  constructor(config?: WalletProviderConfig) {
    this.networkPassphrase =
      config?.networkPassphrase ?? "Test SDF Network ; September 2015";

    if (config?.walletConnectProjectId) {
      this.walletConnect = new WalletConnectClient({
        projectId: config.walletConnectProjectId,
        networkPassphrase: this.networkPassphrase,
      });
    }
  }

  /**
   * Connect to a wallet of the specified type.
   */
  async connect(type: WalletType): Promise<WalletConnection> {
    switch (type) {
      case "albedo":
        return this.connectAlbedo();
      case "walletconnect":
        return this.connectWalletConnect();
      case "freighter":
        return this.connectFreighter();
      default:
        throw new Error(`Unsupported wallet type: ${type}`);
    }
  }

  /**
   * Disconnect the current wallet.
   */
  async disconnect(): Promise<void> {
    if (this.connection?.type === "walletconnect" && this.walletConnect) {
      await this.walletConnect.disconnect();
    }
    this.connection = null;
  }

  /**
   * Sign a transaction with the connected wallet.
   */
  async signTransaction(xdr: string): Promise<string> {
    if (!this.connection) {
      throw new Error("No wallet connected. Call connect() first.");
    }

    switch (this.connection.type) {
      case "albedo": {
        const albedo = this.getAlbedo();
        const result: AlbedoSignResult = await albedo.signTransaction(
          xdr,
          this.networkPassphrase,
        );
        return result.signed_envelope_xdr;
      }
      case "walletconnect": {
        if (!this.walletConnect) {
          throw new Error("WalletConnect not configured.");
        }
        return this.walletConnect.signTransaction(
          xdr,
          this.networkPassphrase,
        );
      }
      case "freighter": {
        const { signTransaction } = await import("@stellar/freighter-api");
        const result = await signTransaction(xdr, {
          networkPassphrase: this.networkPassphrase,
        });
        return result.signedTxXdr ?? result.signed_envelope_xdr;
      }
    }
  }

  /**
   * Get the current connection.
   */
  getConnection(): WalletConnection | null {
    return this.connection;
  }

  /**
   * Check if a wallet is connected.
   */
  isConnected(): boolean {
    return this.connection !== null;
  }

  /**
   * Get the connected public key.
   */
  getPublicKey(): string | null {
    return this.connection?.publicKey ?? null;
  }

  /**
   * Check which wallet types are available in the current environment.
   */
  getAvailableWallets(): WalletType[] {
    const available: WalletType[] = [];

    // Albedo is always available (web-based)
    available.push("albedo");

    // WalletConnect needs a project ID
    if (this.walletConnect) {
      available.push("walletconnect");
    }

    // Freighter requires the browser extension
    if (typeof window !== "undefined" && "freighter" in window) {
      available.push("freighter");
    }

    return available;
  }

  // ── Private helpers ──────────────────────────────────────────────────

  private async connectAlbedo(): Promise<WalletConnection> {
    const albedo = this.getAlbedo();
    const result = await albedo.getPublicKey();

    if (!result.approved) {
      throw new Error("Albedo connection was rejected by the user.");
    }

    this.connection = { type: "albedo", publicKey: result.pubkey };
    return this.connection;
  }

  private async connectWalletConnect(): Promise<WalletConnection> {
    if (!this.walletConnect) {
      throw new Error(
        "WalletConnect not configured. Provide walletConnectProjectId.",
      );
    }

    const session: WCSession = await this.walletConnect.connect();
    this.connection = {
      type: "walletconnect",
      publicKey: session.publicKey,
    };
    return this.connection;
  }

  private async connectFreighter(): Promise<WalletConnection> {
    try {
      const freighter = await import("@stellar/freighter-api");

      const isAllowed = await freighter.isAllowed();
      if (!isAllowed) {
        await freighter.setAllowed();
      }

      const { address } = await freighter.getAddress();
      this.connection = { type: "freighter", publicKey: address };
      return this.connection;
    } catch {
      throw new Error(
        "Freighter not available. Install the Freighter browser extension.",
      );
    }
  }

  private getAlbedo(): AlbedoClient {
    if (!this.albedo) {
      this.albedo = new AlbedoClient();
    }
    return this.albedo;
  }
}
