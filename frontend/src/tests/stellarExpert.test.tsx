import React from 'react';
import { render, screen, within } from '@testing-library/react';
import { StellarExpertClient, StellarExpertError, explorerUrl } from '../services/stellarExpert';
import { ExplorerPanel } from '../views/ExplorerPanel';

const API = 'https://api.stellar.expert/explorer/testnet';
const ACCOUNT = 'GAIH3ULLFQ4DGSECF2AR555KZ4KNDGEKN4AFI4SU2M7B43MGK3QJZNSR';
const HASH = 'a0dfa2b8c6bd9ec986d73d18d5e228dd25793beb818a2a86fbf4d7135cfa3fa0';

const reply = (status: number, body: unknown) =>
  ({ ok: status >= 200 && status < 300, status, statusText: 'status text', json: async () => body }) as Response;

function client(...responses: Array<Response | Error>) {
  const fetchImpl = jest.fn(async (_input: RequestInfo | URL) => {
    const next = responses.shift();
    if (!next) throw new Error('unexpected request');
    if (next instanceof Error) throw next;
    return next;
  });
  return { expert: new StellarExpertClient({ fetchImpl: fetchImpl as unknown as typeof fetch }), fetchImpl };
}

const day = (ts: number, transactions: number) => ({
  ledgers: 5783,
  operations: 79891,
  transactions,
  failed_transactions: 13786,
  payments: 87775,
  active_accounts: 6898,
  accounts: 5020347,
  trustlines: 5963009,
  avg_ledger_time: 5,
  total_xlm: '1000000000000000000',
  fee_pool: '49898969654197',
  ts,
});

describe('explorerUrl', () => {
  test('builds stellar.expert links for each entity', () => {
    expect(explorerUrl('testnet', 'account', ACCOUNT)).toBe(`https://stellar.expert/explorer/testnet/account/${ACCOUNT}`);
    expect(explorerUrl('public', 'tx', HASH)).toBe(`https://stellar.expert/explorer/public/tx/${HASH}`);
    expect(explorerUrl('testnet', 'ledger', 5844873)).toBe('https://stellar.expert/explorer/testnet/ledger/5844873');
    expect(explorerUrl('testnet', 'asset', `USDC-${ACCOUNT}`)).toBe(
      `https://stellar.expert/explorer/testnet/asset/USDC-${ACCOUNT}`,
    );
  });
});

describe('StellarExpertClient', () => {
  test('looks up an account', async () => {
    const account = { address: ACCOUNT, created: 1765992782, payments: 12, trades: 3 };
    const { expert, fetchImpl } = client(reply(200, account));

    await expect(expert.getAccount(ACCOUNT)).resolves.toEqual(account);
    expect(fetchImpl).toHaveBeenCalledWith(`${API}/account/${ACCOUNT}`);
  });

  test('pages through account transaction history', async () => {
    const { expert, fetchImpl } = client(
      reply(200, {
        _links: { next: { href: `/explorer/testnet/tx?sort=id&account=${ACCOUNT}&order=desc&limit=1&cursor=251` } },
        _embedded: { records: [{ id: '251', hash: HASH, ledger: 5844873, ts: 1795248006, paging_token: '251' }] },
      }),
    );

    const page = await expert.getAccountHistory(ACCOUNT, { limit: 1, cursor: '300' });
    expect(page.records.map((r) => r.hash)).toEqual([HASH]);
    expect(page.next).toBe('251');
    expect(fetchImpl).toHaveBeenCalledWith(`${API}/tx?account=${ACCOUNT}&order=desc&limit=1&cursor=300`);
  });

  test('reads asset information', async () => {
    const asset = { asset: 'XLM', supply: '1000000000000000000', trustlines: { total: 1, authorized: 0, funded: 1 }, payments: 9, trades: 2 };
    const { expert, fetchImpl } = client(reply(200, asset));

    await expect(expert.getAsset('XLM')).resolves.toEqual(asset);
    expect(fetchImpl).toHaveBeenCalledWith(`${API}/asset/XLM`);
  });

  test('returns the most recent day of network statistics', async () => {
    const { expert } = client(reply(200, [day(1795132800, 10), day(1795219200, 30), day(1795046400, 20)]));

    await expect(expert.getNetworkStats()).resolves.toMatchObject({ ts: 1795219200, transactions: 30 });
  });

  test('maps API errors, including the paid public-network API', async () => {
    const { expert } = client(
      reply(400, { error: 'Bad request. Invalid parameter: "account". Invalid account public key.', status: 400 }),
      reply(402, { error: 'Payment Required. Payment required', status: 402 }),
      new Error('offline'),
    );

    await expect(expert.getAccount('GBAD')).rejects.toMatchObject({ status: 400, message: expect.stringContaining('Invalid account') });
    const paid = await expert.getAsset('XLM').catch((e) => e);
    expect(paid).toBeInstanceOf(StellarExpertError);
    expect(paid.status).toBe(402);
    expect(paid.message).toContain('testnet is free');
    await expect(expert.getNetworkStats()).rejects.toMatchObject({ status: 0 });
  });
});

describe('ExplorerPanel', () => {
  test('displays stats and links account, transactions and contract to the explorer', () => {
    render(
      <ExplorerPanel
        network="testnet"
        stats={day(1795219200, 68260)}
        account={{ address: ACCOUNT, created: 1765992782, payments: 4823626, trades: 0 }}
        transactions={[{ id: '251', hash: HASH, ledger: 5844873, ts: 1795248006, paging_token: '251' }]}
        contractId="CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC"
      />,
    );

    const stats = screen.getByLabelText('Network activity on 2026-11-21');
    expect(within(stats).getByText('68,260')).toBeTruthy();
    expect(screen.getByText(/4,823,626 payments/)).toBeTruthy();

    const hrefs = screen.getAllByRole('link').map((a) => a.getAttribute('href'));
    expect(hrefs).toEqual([
      'https://stellar.expert/explorer/testnet/contract/CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC',
      `https://stellar.expert/explorer/testnet/account/${ACCOUNT}`,
      `https://stellar.expert/explorer/testnet/tx/${HASH}`,
      'https://stellar.expert/explorer/testnet/ledger/5844873',
    ]);
  });

  test('shows the API error instead of failing silently', () => {
    render(<ExplorerPanel network="public" error="Payment Required" />);
    expect(screen.getByRole('alert').textContent).toContain('Payment Required');
  });
});
