import React from 'react';
import {
  ExpertAccount,
  ExpertTransaction,
  NetworkStats,
  StellarNetwork,
  explorerUrl,
} from '../services/stellarExpert';

export interface ExplorerPanelProps {
  network: StellarNetwork;
  /** Latest `StellarExpertClient.getNetworkStats()` result. */
  stats?: NetworkStats;
  /** `StellarExpertClient.getAccount()` result for the connected player. */
  account?: ExpertAccount;
  /** `StellarExpertClient.getAccountHistory()` records. */
  transactions?: ExpertTransaction[];
  /** Game contract id, linked to its explorer page. */
  contractId?: string;
  /** Error message from the last StellarExpert call, if any. */
  error?: string;
}

const linkProps = { target: '_blank', rel: 'noopener noreferrer' } as const;

const shortHash = (hash: string) => `${hash.slice(0, 8)}…${hash.slice(-8)}`;

const formatDate = (unixSeconds: number) => new Date(unixSeconds * 1000).toISOString().slice(0, 10);

/** Network stats, player account summary and explorer links from StellarExpert (#714). */
export const ExplorerPanel: React.FC<ExplorerPanelProps> = ({
  network,
  stats,
  account,
  transactions = [],
  contractId,
  error,
}) => (
  <section aria-label="Stellar explorer" style={{ padding: 16, borderRadius: 8, background: '#0F172A', color: '#F8FAFC' }}>
    <h2 style={{ marginTop: 0, fontSize: '1.1rem' }}>Stellar network ({network})</h2>

    {error && <p role="alert">StellarExpert is unavailable: {error}</p>}

    {stats && (
      <dl aria-label={`Network activity on ${formatDate(stats.ts)}`}>
        <dt>Transactions</dt>
        <dd>{stats.transactions.toLocaleString('en-US')}</dd>
        <dt>Failed transactions</dt>
        <dd>{stats.failed_transactions.toLocaleString('en-US')}</dd>
        <dt>Operations</dt>
        <dd>{stats.operations.toLocaleString('en-US')}</dd>
        <dt>Active accounts</dt>
        <dd>{stats.active_accounts.toLocaleString('en-US')}</dd>
        <dt>Average ledger time</dt>
        <dd>{stats.avg_ledger_time}s</dd>
      </dl>
    )}

    {contractId && (
      <p>
        Game contract:{' '}
        <a href={explorerUrl(network, 'contract', contractId)} {...linkProps}>
          {shortHash(contractId)}
        </a>
      </p>
    )}

    {account && (
      <div>
        <h3 style={{ fontSize: '1rem' }}>
          Account{' '}
          <a href={explorerUrl(network, 'account', account.address)} {...linkProps}>
            {shortHash(account.address)}
          </a>
        </h3>
        <p>
          Created {formatDate(account.created)} · {account.payments.toLocaleString('en-US')} payments ·{' '}
          {account.trades.toLocaleString('en-US')} trades
        </p>
      </div>
    )}

    {transactions.length > 0 && (
      <ul aria-label="Recent transactions">
        {transactions.map((tx) => (
          <li key={tx.id}>
            <a href={explorerUrl(network, 'tx', tx.hash)} {...linkProps}>
              {shortHash(tx.hash)}
            </a>{' '}
            in ledger{' '}
            <a href={explorerUrl(network, 'ledger', tx.ledger)} {...linkProps}>
              {tx.ledger}
            </a>
          </li>
        ))}
      </ul>
    )}
  </section>
);
