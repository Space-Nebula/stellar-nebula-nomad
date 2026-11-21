# StellarExpert Integration

[StellarExpert](https://stellar.expert) is a Stellar block explorer with a public JSON API. The game uses it in two places:

| Layer | File | Role |
|-------|------|------|
| Frontend | `frontend/src/services/stellarExpert.ts` | API client (accounts, transaction history, assets, network statistics) and explorer link builder |
| Frontend | `frontend/src/views/ExplorerPanel.tsx` | Displays network stats, the player's account summary and recent transactions, all linked to the explorer |
| Contract | `integrations/stellar_expert.rs` | Emits `expert` events (metadata, interactions, volume, activity, stats snapshots) for off-chain analytics. A Soroban contract cannot make HTTP calls, so it never reads the API itself. |

## Networks and cost

The API base is `https://api.stellar.expert/explorer/{network}/`, with `network` set to `testnet` or `public`.

- **Testnet** requests are free.
- **Public network** requests currently answer `402 Payment Required` with an [x402](https://www.x402.org) `payment-required` header: the mainnet API is pay-per-request. The client reports this as `StellarExpertError` with `status: 402`. Explorer links (`explorerUrl`) are ordinary web pages and work on both networks.

## Client

```ts
import { StellarExpertClient, explorerUrl } from '../services/stellarExpert';

const expert = new StellarExpertClient({ network: 'testnet' });
```

| Method | API path | Returns |
|--------|----------|---------|
| `getAccount(address)` | `account/{address}` | `address`, `created`, `creator`, `payments`, `trades`, `activity` |
| `getAccountHistory(address, { limit, cursor, order })` | `tx?account={address}&order&limit&cursor` | `{ records, next }`. Pass `next` back as `cursor` for the following page; an empty `records` array means the end. Newest first by default. |
| `getAsset(asset)` | `asset/{asset}` | `supply`, `trustlines`, `payments`, `trades`, `price`, `volume7d`, `contract`. `asset` is `XLM` or `CODE-ISSUER`. |
| `getNetworkStats()` | `ledger/ledger-stats` | The most recent day: `transactions`, `failed_transactions`, `operations`, `payments`, `active_accounts`, `accounts`, `avg_ledger_time`, `ts`, ... |

Every failure throws `StellarExpertError`:

| `status` | Meaning |
|----------|---------|
| `0` | Network failure (no response) |
| `400` | Invalid parameter, e.g. a malformed account key. `message` holds the API's `error` text. |
| `402` | Paid public-network API |
| `404` | Unknown entity or endpoint |

### Explorer links

```ts
explorerUrl('testnet', 'account', address);   // https://stellar.expert/explorer/testnet/account/G...
explorerUrl('testnet', 'tx', hash);           // .../tx/<hash>
explorerUrl('testnet', 'contract', contractId);
explorerUrl('testnet', 'asset', 'USDC-G...');
explorerUrl('testnet', 'ledger', 5844873);
explorerUrl('testnet', 'op', operationId);
```

## Explorer panel

`ExplorerPanel` takes the client's results as props, like the other views:

```tsx
const [stats, account, history] = await Promise.all([
  expert.getNetworkStats(),
  expert.getAccount(player),
  expert.getAccountHistory(player, { limit: 5 }),
]);

<ExplorerPanel
  network="testnet"
  stats={stats}
  account={account}
  transactions={history.records}
  contractId={CONTRACT_ID}
/>
```

Pass `error={err.message}` when a call fails; the panel shows it in an alert.

## Contract events

| Function | Topics | Data |
|----------|--------|------|
| `register_expert_metadata` | `("expert", "meta")` | `ExpertMetadata` |
| `emit_interaction_event` | `("expert", "action")` | `(user, action, value)` |
| `emit_volume_event` | `("expert", "volume")` | `(user, operation, amount, timestamp)` |
| `emit_activity_event` | `("expert", "active")` | `(user, action, timestamp)` |
| `emit_stats_snapshot` | `("expert", "stats")` | `ContractStats` |

## Tests

```bash
cargo test --lib stellar_expert                                   # contract events
cd frontend && npx jest src/tests/stellarExpert.test.tsx          # client, links and panel (fetch is mocked)
```
