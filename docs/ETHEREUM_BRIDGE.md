# Ethereum Bridge

`src/bridge/stellar_ethereum.rs` moves tokens and messages between Stellar and Ethereum. It is exposed on `NebulaNomadContract` as the `bridge_*` entry points. The Bridge view (`frontend/src/views/Bridge.tsx`) is the player-facing form.

## Model

The bridge uses **lock and release** on Stellar:

- Each bridged asset is a Stellar token contract (for example a Stellar Asset Contract) that the game contract holds in custody.
- On Ethereum, a wrapped token is minted when tokens are locked here, and burned to bring them back.
- The **bridge admin is the relayer**. It watches both chains and reports Ethereum events to the contract. Every relayer call needs the admin's signature.

The Ethereum contracts and the relayer process are not part of this repository. They must follow the event and call flow below.

## Flows

### Stellar → Ethereum

| Step | Call | Who | Effect |
|------|------|-----|--------|
| 1 | `bridge_initiate(player, asset, chain_id, amount, eth_recipient)` | player | Moves `amount` from the player into custody. Creates a `Pending` request. Emits `("bridge", "initiated")` with `(request_id, player, asset, chain_id, amount, fee, eth_recipient)`. |
| 2 | Relayer mints `amount - fee` to `eth_recipient` on Ethereum | relayer | |
| 3 | `bridge_confirm(admin, request_id, eth_mint_tx_hash, confirmations)` | relayer | Needs `confirmations >= min_confirmations`. Adds `amount - fee` to the asset's `locked` and `fee` to its `fees`. Status `Confirmed`. |
| 4 | `bridge_complete(admin, request_id)` | relayer | Status `Completed`. |

A `Pending` request that never reaches Ethereum is refunded in full (fee included):

- `bridge_cancel(player, request_id)`: by the player. Status `Cancelled`.
- `bridge_fail(admin, request_id)`: by the relayer, for example when the mint reverted. Status `Failed`.

After confirmation a request can no longer be refunded, because the wrapped tokens exist on Ethereum.

### Ethereum → Stellar

1. The holder burns wrapped tokens on Ethereum.
2. The relayer waits for confirmations and calls `bridge_release(admin, eth_burn_tx_hash, eth_sender, asset, chain_id, recipient, amount, confirmations)`.
3. The contract checks the hash was never released, `confirmations >= min_confirmations`, and `amount <= locked`. It sends `amount - fee` to `recipient`, removes `amount` from `locked`, adds the fee to `fees`, stores a `Completed` `EthToStellar` request and emits `("bridge", "released")`.

A second call with the same burn hash fails with `AlreadyProcessed` (`bridge_is_eth_tx_processed` reports it). Releases are keyed by transaction hash, so the Ethereum contract must allow one bridge burn per transaction. Replay keys are persistent storage, so they are archived, never deleted.

### Messages

| Call | Who | Effect |
|------|-----|--------|
| `bridge_send_message(sender, chain_id, eth_target, payload)` | any signed account | Returns an increasing nonce and emits `("bridge", "msg_out")` with `(nonce, sender, chain_id, eth_target, payload)` for the relayer to deliver. |
| `bridge_receive_message(admin, chain_id, nonce, eth_sender, recipient, payload)` | relayer | Accepts each `(chain_id, nonce)` once and emits `("bridge", "msg_in")` with `(chain_id, nonce, eth_sender, recipient, payload)`. |
| `bridge_is_message_processed(chain_id, nonce)` | anyone | Replay check. |

Payloads must be 1 to `MAX_MESSAGE_BYTES` (1024) bytes.

## Fees

- The fee is `floor(amount * fee_bps / 10_000)`. The default is 30 bps (0.3%). `bridge_update_config` rejects anything above `MAX_BRIDGE_FEE_BPS` (100 bps, 1%).
- The fee is charged in both directions and only on transfers that go through. Refunds return the full amount.
- Fees accrue per asset in `WrappedAsset.fees`. `bridge_withdraw_fees(admin, asset, chain_id, to)` pays them out and resets the counter.

## State synchronization

For every asset, `bridge_get_asset(symbol, chain_id)` returns:

- `locked`: tokens backing the wrapped supply. **It must equal the wrapped token's total supply on Ethereum.** The relayer should compare the two and pause the bridge if they drift.
- `fees`: fees not yet withdrawn.

The contract's token balance always equals `locked + fees +` the amounts of `Pending` requests.

Requests are stored per ID (`bridge_get_request`). Each request records the Ethereum transaction (`dest_tx_hash` for mints, `source_tx_hash` for burns) and the confirmation count the relayer reported. Each player can have at most `MAX_PENDING_REQUESTS` (100) pending requests. The cap is per player, so one account cannot fill a shared queue and block everyone else.

## Security

| Control | Where |
|---------|-------|
| One-time initialization: a second `bridge_initialize` fails with `AlreadyInitialized` | `initialize_bridge` |
| Relayer-only calls check the stored admin | `require_admin` |
| Global pause (`bridge_set_paused`) and per-asset switch (`bridge_set_asset_enabled`) | new transfers and messages |
| Replay protection for burn hashes and message nonces | persistent `StellarEthProcessedTx` / `StellarEthProcessedMsg` keys |
| Minimum Ethereum confirmations (default 12) | `bridge_confirm`, `bridge_release` |
| Per-transfer limit (`max_amount`) and non-zero Ethereum addresses | `bridge_initiate`, `bridge_release` |
| Releases can never exceed `locked` | `bridge_release` |
| Fee cap of 1% | `bridge_update_config` |

The relayer key is a single point of trust. Use a multisig account as the admin.

Like the contract's other `initialize_*` entry points, `bridge_initialize` makes its first caller the admin. Call it in the deploy script right after deployment, and check `bridge_get_config().admin` before announcing the bridge. Once set, the admin can't be replaced.

Errors are `BridgeError` codes 90–106; see [ERROR_CODES.md](ERROR_CODES.md#bridge-stellar-ethereum-bridgeerror).

## Frontend

`Bridge` takes the contract state as props and calls back for writes:

```tsx
<Bridge
  assets={[{ symbol: 'XLM', chainId: 1, enabled: true, balance }]}
  feeBps={config.fee_bps}
  maxAmount={config.max_amount}
  paused={paused}
  requests={requests}
  onBridge={({ asset, chainId, amount, ethRecipient }) => /* bridge_initiate */ {}}
  onCancel={(requestId) => /* bridge_cancel */ {}}
/>
```

The view checks amounts and addresses with the same rules as the contract (`frontend/src/utils/bridge.ts`), shows the fee and the amount received on Ethereum before submitting, and offers **Cancel** on pending outbound requests.

## Tests

```bash
cargo test --lib stellar_ethereum                       # contract flows with a real Stellar Asset Contract
cd frontend && npx jest src/tests/bridge.test.tsx       # rule mirrors and the Bridge view
```
