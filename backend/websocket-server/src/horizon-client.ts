/**
 * Horizon REST client (#715): transaction submission and tracking, account
 * lookups, account monitoring over Horizon's Server-Sent Events streams, and
 * typed errors parsed from Horizon's Problem Details responses.
 *
 * Uses the platform `fetch` (Node 18+), like mobile/src/services/StellarService.ts.
 * Transactions are built and signed by the caller (e.g. with @stellar/stellar-sdk);
 * this client only moves signed envelopes and reads results.
 */

export const HORIZON_TESTNET_URL = 'https://horizon-testnet.stellar.org';
export const HORIZON_PUBLIC_URL = 'https://horizon.stellar.org';

export interface HorizonBalance {
  asset_type: string;
  balance: string;
  asset_code?: string;
  asset_issuer?: string;
}

export interface HorizonAccount {
  id: string;
  sequence: string;
  balances: HorizonBalance[];
}

export interface HorizonTransaction {
  id: string;
  hash: string;
  ledger: number;
  successful: boolean;
  paging_token: string;
  created_at: string;
  source_account: string;
}

/** `extras.result_codes` of a 400 `transaction_failed` response. */
export interface HorizonResultCodes {
  transaction?: string;
  operations?: string[];
}

/** A failed Horizon call. `status` is the HTTP status, or 0 when the request never got a response. */
export class HorizonError extends Error {
  constructor(
    readonly status: number,
    readonly title: string,
    readonly detail?: string,
    readonly resultCodes?: HorizonResultCodes,
  ) {
    super(detail ? `${title}: ${detail}` : title);
    this.name = 'HorizonError';
  }
}

export interface HorizonClientOptions {
  /** Defaults to the SDF testnet Horizon. */
  horizonUrl?: string;
  fetchImpl?: typeof fetch;
}

export interface TrackOptions {
  timeoutMs?: number;
  intervalMs?: number;
}

export interface MonitorOptions {
  /** Paging token to resume from; `now` (default) streams only new transactions. */
  cursor?: string;
  onError?: (error: HorizonError) => void;
}

export interface StreamHandle {
  close(): void;
  /** Settles once the stream is closed or has failed. */
  done: Promise<void>;
}

async function toError(res: Response): Promise<HorizonError> {
  let problem: { title?: string; detail?: string; extras?: { result_codes?: HorizonResultCodes } } = {};
  try {
    problem = (await res.json()) as typeof problem;
  } catch {
    // Body is not Problem Details JSON; fall back to the status line.
  }
  return new HorizonError(
    res.status,
    problem.title ?? res.statusText,
    problem.detail,
    problem.extras?.result_codes,
  );
}

const isRetryable = (status: number) => status === 0 || status === 404 || status === 429 || status >= 500;

const sleep = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Pause before reopening a stream Horizon closed, so a server that keeps closing it is not hammered. */
const RECONNECT_DELAY_MS = 1_000;

export class HorizonClient {
  private readonly baseUrl: string;
  private readonly fetchImpl: typeof fetch;

  constructor(options: HorizonClientOptions = {}) {
    this.baseUrl = (options.horizonUrl ?? HORIZON_TESTNET_URL).replace(/\/+$/, '');
    this.fetchImpl = options.fetchImpl ?? fetch;
  }

  getAccount(accountId: string): Promise<HorizonAccount> {
    return this.request(`/accounts/${encodeURIComponent(accountId)}`);
  }

  getTransaction(hash: string): Promise<HorizonTransaction> {
    return this.request(`/transactions/${encodeURIComponent(hash)}`);
  }

  /**
   * Submit a signed transaction envelope (base64 XDR). Resolves with the
   * ledgered transaction; a rejected transaction throws a HorizonError whose
   * `resultCodes` holds Horizon's `tx_*` / `op_*` codes.
   */
  submitTransaction(envelopeXdr: string): Promise<HorizonTransaction> {
    // Form body, as @stellar/stellar-sdk sends it: a Soroban envelope can be too long for a URL.
    return this.request('/transactions', { method: 'POST', body: new URLSearchParams({ tx: envelopeXdr }) });
  }

  /**
   * Poll until `hash` is in a ledger. Not-found, network, 429 and 5xx answers
   * are retried until `timeoutMs`, then HorizonError(408) is thrown.
   */
  async trackTransaction(
    hash: string,
    { timeoutMs = 30_000, intervalMs = 1_000 }: TrackOptions = {},
  ): Promise<HorizonTransaction> {
    const deadline = Date.now() + timeoutMs;
    for (;;) {
      try {
        return await this.getTransaction(hash);
      } catch (err) {
        if (!(err instanceof HorizonError) || !isRetryable(err.status)) throw err;
      }
      if (Date.now() + intervalMs > deadline) {
        throw new HorizonError(408, 'Transaction not found before timeout', hash);
      }
      await sleep(intervalMs);
    }
  }

  /**
   * Stream the account's transactions to `onTransaction`. When Horizon ends the
   * stream, it reconnects from the last paging token, so no record is missed or
   * repeated. HTTP and network failures stop the stream and go to `onError`.
   */
  monitorAccount(
    accountId: string,
    onTransaction: (tx: HorizonTransaction) => void,
    { cursor = 'now', onError }: MonitorOptions = {},
  ): StreamHandle {
    const controller = new AbortController();
    let position = cursor;

    const run = async () => {
      while (!controller.signal.aborted) {
        const url =
          `${this.baseUrl}/accounts/${encodeURIComponent(accountId)}/transactions` +
          `?cursor=${encodeURIComponent(position)}`;
        let res: Response;
        try {
          res = await this.fetchImpl(url, {
            headers: { Accept: 'text/event-stream' },
            signal: controller.signal,
          });
        } catch (err) {
          throw new HorizonError(0, 'Network error', (err as Error).message);
        }
        if (!res.ok || !res.body) throw await toError(res);
        await readEvents(res.body, (data) => {
          const record = parseRecord(data);
          if (!record) return;
          position = record.paging_token;
          onTransaction(record);
        });
        if (!controller.signal.aborted) await sleep(RECONNECT_DELAY_MS);
      }
    };

    const done = run().catch((err) => {
      if (controller.signal.aborted) return;
      onError?.(err instanceof HorizonError ? err : new HorizonError(0, 'Stream error', String(err)));
    });
    return { close: () => controller.abort(), done };
  }

  private async request<T>(path: string, init: RequestInit = {}): Promise<T> {
    let res: Response;
    try {
      res = await this.fetchImpl(this.baseUrl + path, {
        ...init,
        headers: { Accept: 'application/json' },
      });
    } catch (err) {
      throw new HorizonError(0, 'Network error', (err as Error).message);
    }
    if (!res.ok) throw await toError(res);
    return (await res.json()) as T;
  }
}

/** Horizon sends a JSON string ("hello") on connect; only transaction records have a paging token. */
function parseRecord(data: string): HorizonTransaction | null {
  try {
    const value = JSON.parse(data);
    return value && typeof value === 'object' && typeof value.paging_token === 'string' ? value : null;
  } catch {
    return null;
  }
}

/** Read an SSE body and hand each event's `data` to `onData`. */
async function readEvents(body: ReadableStream<Uint8Array>, onData: (data: string) => void): Promise<void> {
  const reader = body.getReader();
  const decoder = new TextDecoder();
  let buffer = '';
  for (;;) {
    const { value, done } = await reader.read();
    if (done) return;
    buffer += decoder.decode(value, { stream: true }).replace(/\r\n?/g, '\n');
    let end: number;
    while ((end = buffer.indexOf('\n\n')) !== -1) {
      const data = buffer
        .slice(0, end)
        .split('\n')
        .filter((line) => line.startsWith('data:'))
        .map((line) => line.slice(5).replace(/^ /, ''))
        .join('\n');
      buffer = buffer.slice(end + 2);
      if (data) onData(data);
    }
  }
}
