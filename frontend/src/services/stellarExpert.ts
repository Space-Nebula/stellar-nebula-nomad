/**
 * StellarExpert explorer API client and explorer links (#714).
 *
 * API base: https://api.stellar.expert/explorer/{network}/
 * Testnet calls are free. Public-network calls answer 402 Payment Required
 * (x402 pay-per-request); that surfaces as a StellarExpertError with status 402.
 */

export type StellarNetwork = 'testnet' | 'public';

export const STELLAR_EXPERT_API = 'https://api.stellar.expert/explorer';
export const STELLAR_EXPERT_SITE = 'https://stellar.expert/explorer';

export type ExplorerEntity = 'account' | 'tx' | 'asset' | 'contract' | 'ledger' | 'op';

/** Link to an entity's page on stellar.expert. Assets use `XLM` or `CODE-ISSUER`. */
export function explorerUrl(network: StellarNetwork, entity: ExplorerEntity, id: string | number): string {
  return `${STELLAR_EXPERT_SITE}/${network}/${entity}/${encodeURIComponent(String(id))}`;
}

export interface ExpertAccount {
  address: string;
  /** Unix seconds. */
  created: number;
  creator?: string;
  payments: number;
  trades: number;
  activity?: { yearly?: string; monthly?: string };
}

export interface ExpertTransaction {
  id: string;
  hash: string;
  ledger: number;
  /** Unix seconds. */
  ts: number;
  paging_token: string;
}

export interface ExpertAsset {
  asset: string;
  code?: string;
  supply: string;
  trustlines: { total: number; authorized: number; funded: number };
  payments: number;
  trades: number;
  price?: number;
  volume7d?: number;
  contract?: string;
}

/** One day of network activity (`ledger/ledger-stats` row). */
export interface NetworkStats {
  ledgers: number;
  operations: number;
  transactions: number;
  failed_transactions: number;
  payments: number;
  active_accounts: number;
  accounts: number;
  trustlines: number;
  avg_ledger_time: number;
  total_xlm: string;
  fee_pool: string;
  /** Unix seconds (start of the day). */
  ts: number;
}

export interface ExpertPage<T> {
  records: T[];
  /** Cursor for the following page. An empty `records` array means the end. */
  next?: string;
}

export interface HistoryOptions {
  limit?: number;
  cursor?: string;
  order?: 'asc' | 'desc';
}

export class StellarExpertError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = 'StellarExpertError';
  }
}

export interface StellarExpertClientOptions {
  network?: StellarNetwork;
  fetchImpl?: typeof fetch;
}

export class StellarExpertClient {
  readonly network: StellarNetwork;
  private readonly fetchImpl: typeof fetch;

  constructor({ network = 'testnet', fetchImpl }: StellarExpertClientOptions = {}) {
    this.network = network;
    this.fetchImpl = fetchImpl ?? ((input, init) => fetch(input, init));
  }

  getAccount(address: string): Promise<ExpertAccount> {
    return this.get(`account/${encodeURIComponent(address)}`);
  }

  /** Transactions involving `address`, newest first by default. */
  async getAccountHistory(
    address: string,
    { limit = 20, cursor, order = 'desc' }: HistoryOptions = {},
  ): Promise<ExpertPage<ExpertTransaction>> {
    const query = new URLSearchParams({ account: address, order, limit: String(limit) });
    if (cursor) query.set('cursor', cursor);
    const page = await this.get<{
      _embedded: { records: ExpertTransaction[] };
      _links?: { next?: { href: string } };
    }>(`tx?${query}`);
    const nextHref = page._links?.next?.href;
    return {
      records: page._embedded.records,
      next: nextHref ? new URLSearchParams(nextHref.split('?')[1]).get('cursor') ?? undefined : undefined,
    };
  }

  /** `asset` is `XLM` or `CODE-ISSUER`. */
  getAsset(asset: string): Promise<ExpertAsset> {
    return this.get(`asset/${encodeURIComponent(asset)}`);
  }

  /** The most recent day of network activity. */
  async getNetworkStats(): Promise<NetworkStats> {
    const days = await this.get<NetworkStats[]>('ledger/ledger-stats');
    if (days.length === 0) throw new StellarExpertError(404, 'No network statistics available');
    return days.reduce((latest, day) => (day.ts > latest.ts ? day : latest));
  }

  private async get<T>(path: string): Promise<T> {
    let res: Response;
    try {
      res = await this.fetchImpl(`${STELLAR_EXPERT_API}/${this.network}/${path}`);
    } catch (err) {
      throw new StellarExpertError(0, `Network error: ${(err as Error).message}`);
    }
    if (!res.ok) {
      let message = res.statusText;
      try {
        message = ((await res.json()) as { error?: string }).error ?? message;
      } catch {
        // Body is not JSON; keep the status text.
      }
      if (res.status === 402) message = `${message} (the public-network API is paid; testnet is free)`;
      throw new StellarExpertError(res.status, message);
    }
    return (await res.json()) as T;
  }
}
