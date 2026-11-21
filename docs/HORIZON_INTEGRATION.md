# Horizon Integration

Stellar Nebula Nomad talks to [Horizon](https://developers.stellar.org/docs/data/apis/horizon) in two places:

| Layer | File | Role |
|-------|------|------|
| Contract | `integrations/horizon_client.rs` | Emits `horizon` events that off-chain indexers pick up. A Soroban contract cannot make HTTP calls, so it never reads Horizon itself. |
| Backend | `backend/websocket-server/src/horizon-client.ts` | Calls the Horizon REST API: submits transactions, tracks them, reads accounts, and streams account activity. |

> Horizon is marked "nearing end-of-life" by SDF in favour of [Stellar RPC](https://developers.stellar.org/docs/data/apis/migrate-from-horizon-to-rpc). It still receives compatibility updates, and every endpoint used here is part of the current API reference.

## Backend client

```ts
import { HorizonClient, HorizonError } from './horizon-client';

const horizon = new HorizonClient(); // testnet by default
// new HorizonClient({ horizonUrl: HORIZON_PUBLIC_URL }) for mainnet
```

| Method | Horizon endpoint | Notes |
|--------|------------------|-------|
| `getAccount(id)` | `GET /accounts/{id}` | `id`, `sequence`, `balances[]` |
| `getTransaction(hash)` | `GET /transactions/{hash}` | |
| `submitTransaction(xdr)` | `POST /transactions` with form field `tx=<base64 XDR>` | Takes an envelope the caller already built and signed. Sent in the body, as `@stellar/stellar-sdk` does, because Soroban envelopes can be too long for a URL. Resolves with the ledgered transaction. |
| `trackTransaction(hash, { timeoutMs, intervalMs })` | `GET /transactions/{hash}` (polled) | Retries 404 (not ledgered yet), network failures, 429 and 5xx. Stops on other errors. Throws `HorizonError` 408 after `timeoutMs` (default 30 s). |
| `monitorAccount(id, onTx, { cursor, onError })` | `GET /accounts/{id}/transactions` with `Accept: text/event-stream` | Server-Sent Events stream. `cursor` defaults to `now` (new transactions only). Returns `{ close, done }`. |

### Operation submission and tracking

```ts
try {
  const tx = await horizon.submitTransaction(signedEnvelopeXdr);
  console.log('ledgered in', tx.ledger, 'successful:', tx.successful);
} catch (err) {
  if (err instanceof HorizonError && err.resultCodes) {
    console.error(err.resultCodes.transaction, err.resultCodes.operations); // e.g. tx_failed, ['op_underfunded']
  }
}

// When the transaction was handed to another submitter (relayer, wallet):
const tx = await horizon.trackTransaction(hash, { timeoutMs: 60_000 });
```

### Account monitoring

```ts
const stream = horizon.monitorAccount(playerAddress, (tx) => {
  console.log('new transaction', tx.hash, tx.paging_token);
}, {
  onError: (err) => console.error('stream stopped', err.status, err.message),
});

// later
stream.close();
```

When Horizon closes a stream, the client waits one second and reopens it from the last `paging_token` it delivered, so records are neither skipped nor repeated. An HTTP error (for example 404 for an unfunded account) or a network failure stops the stream and is passed to `onError`. Every streamed record counts against Horizon's rate limit.

### Errors

Every failure is a `HorizonError`:

| Field | Source |
|-------|--------|
| `status` | HTTP status; `0` when no response arrived (network failure); `408` for a `trackTransaction` timeout |
| `title`, `detail` | Horizon's [Problem Details](https://developers.stellar.org/docs/data/apis/horizon/api-reference/errors/response) body, or the status text |
| `resultCodes` | `extras.result_codes` of a `400 transaction_failed`: `{ transaction, operations }` |

Status `429` means the per-IP rate limit was hit (Horizon's default is 3600 requests per hour; providers may set their own). Back off before retrying. Resubmitting an already-ledgered transaction is safe: Horizon returns the original result.

## Contract adapter

`integrations/horizon_client.rs` publishes these events for indexers:

| Function | Topics | Data |
|----------|--------|------|
| `query_account_info` | `("horizon", "query")` | account address |
| `submit_transaction` / `emit_tx_for_indexing` | `("horizon", "tx")` | `(tx_hash, operation)` |
| `stream_events` | `("horizon", "stream")` | `(subscriber, topic, sub_topic)` |
| `unsubscribe_stream` | `("horizon", "unsub")` | subscriber |

## Tests

```bash
cargo test --lib horizon_client                   # contract adapter unit tests
cargo test --test test_horizon_integration        # contract adapter integration tests
cd backend/websocket-server && npx jest           # backend client (fetch is mocked; no network)
```
