import React, { useState } from 'react';
import {
  BridgeRequestView,
  STATUS_TEXT,
  amountError,
  bridgeFee,
  isValidEthAddress,
} from '../utils/bridge';

export interface BridgeAssetOption {
  symbol: string;
  chainId: number;
  enabled: boolean;
  /** Player's balance in the token's base units. */
  balance: number;
}

export interface BridgeTransferInput {
  asset: string;
  chainId: number;
  amount: number;
  ethRecipient: string;
}

export interface BridgeProps {
  /** Registered assets (`bridge_get_asset`) with the player's balances. */
  assets: BridgeAssetOption[];
  /** From `bridge_get_config`. */
  feeBps: number;
  maxAmount: number;
  paused: boolean;
  /** The player's requests (`bridge_get_request`), newest first. */
  requests: BridgeRequestView[];
  /** Calls `bridge_initiate`. */
  onBridge: (transfer: BridgeTransferInput) => void;
  /** Calls `bridge_cancel`. */
  onCancel: (requestId: number) => void;
}

const keyOf = (a: BridgeAssetOption) => `${a.symbol}:${a.chainId}`;

/** Stellar -> Ethereum transfer form and the player's bridge requests (#713). */
export const Bridge: React.FC<BridgeProps> = ({ assets, feeBps, maxAmount, paused, requests, onBridge, onCancel }) => {
  const enabled = assets.filter((a) => a.enabled);
  const [assetKey, setAssetKey] = useState('');
  const [amountText, setAmountText] = useState('');
  const [ethRecipient, setEthRecipient] = useState('');

  // Fall back to the first asset so the form works when `assets` arrives after the first render.
  const asset = enabled.find((a) => keyOf(a) === assetKey) ?? enabled[0];
  const amount = Number(amountText);
  const amountProblem = amountText === '' ? null : amountError(amount, maxAmount, asset?.balance ?? 0);
  const addressProblem =
    ethRecipient === '' || isValidEthAddress(ethRecipient) ? null : 'Enter a 0x-prefixed, non-zero Ethereum address.';
  const ready = !paused && asset !== undefined && amountText !== '' && ethRecipient !== '' && !amountProblem && !addressProblem;
  const fee = ready ? bridgeFee(amount, feeBps) : 0;

  const submit = (event: React.FormEvent) => {
    event.preventDefault();
    if (!ready || !asset) return;
    onBridge({ asset: asset.symbol, chainId: asset.chainId, amount, ethRecipient });
  };

  return (
    <section aria-label="Cross-chain bridge" style={{ padding: 16, borderRadius: 8, background: '#0F172A', color: '#F8FAFC' }}>
      <h1 style={{ marginTop: 0, fontSize: '1.25rem' }}>Cross-Chain Bridge</h1>
      <p>Transfer assets between Stellar and Ethereum.</p>

      {paused && <p role="alert">The bridge is paused. Pending transfers can still be cancelled.</p>}

      <form onSubmit={submit}>
        <label>
          Asset{' '}
          <select value={asset ? keyOf(asset) : ''} onChange={(e) => setAssetKey(e.target.value)}>
            {enabled.map((a) => (
              <option key={keyOf(a)} value={keyOf(a)}>
                {a.symbol} (chain {a.chainId}) · balance {a.balance.toLocaleString('en-US')}
              </option>
            ))}
          </select>
        </label>
        <label>
          Amount{' '}
          <input inputMode="numeric" value={amountText} onChange={(e) => setAmountText(e.target.value.trim())} />
        </label>
        <label>
          Ethereum recipient{' '}
          <input value={ethRecipient} placeholder="0x…" onChange={(e) => setEthRecipient(e.target.value.trim())} />
        </label>

        {(amountProblem || addressProblem) && <p role="alert">{amountProblem ?? addressProblem}</p>}
        {ready && (
          <p aria-live="polite">
            Fee {fee.toLocaleString('en-US')} ({feeBps / 100}%) · Received on Ethereum{' '}
            {(amount - fee).toLocaleString('en-US')}
          </p>
        )}

        <button type="submit" disabled={!ready}>
          Bridge to Ethereum
        </button>
      </form>

      <h2 style={{ fontSize: '1.1rem' }}>Your transfers</h2>
      {requests.length === 0 ? (
        <p>No transfers yet.</p>
      ) : (
        <ul aria-label="Bridge requests">
          {requests.map((r) => (
            <li key={r.requestId}>
              #{r.requestId} · {r.direction === 'StellarToEth' ? 'To Ethereum' : 'From Ethereum'} ·{' '}
              {r.amount.toLocaleString('en-US')} {r.asset} · {STATUS_TEXT[r.status]}
              {r.status === 'Pending' && r.direction === 'StellarToEth' && (
                <button type="button" onClick={() => onCancel(r.requestId)}>
                  Cancel
                </button>
              )}
            </li>
          ))}
        </ul>
      )}
    </section>
  );
};
