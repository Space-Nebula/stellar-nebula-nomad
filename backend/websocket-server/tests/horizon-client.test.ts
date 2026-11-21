import { HorizonClient, HorizonError, HorizonTransaction } from '../src/horizon-client';

const HORIZON = 'https://horizon.test';
const ACCOUNT = 'GAIH3ULLFQ4DGSECF2AR555KZ4KNDGEKN4AFI4SU2M7B43MGK3QJZNSR';

function tx(hash: string, pagingToken: string): HorizonTransaction {
  return {
    id: hash,
    hash,
    ledger: 100,
    successful: true,
    paging_token: pagingToken,
    created_at: '2026-11-21T00:00:00Z',
    source_account: ACCOUNT,
  };
}

const json = (status: number, body: unknown) =>
  new Response(JSON.stringify(body), { status, headers: { 'Content-Type': 'application/json' } });

function client(...responses: Array<Response | Error>) {
  const fetchImpl = jest.fn(async () => {
    const next = responses.shift();
    if (!next) throw new Error('unexpected request');
    if (next instanceof Error) throw next;
    return next;
  });
  return { horizon: new HorizonClient({ horizonUrl: `${HORIZON}/`, fetchImpl }), fetchImpl };
}

describe('HorizonClient', () => {
  test('getAccount reads /accounts/{id}', async () => {
    const account = { id: ACCOUNT, sequence: '42', balances: [{ asset_type: 'native', balance: '10.0000000' }] };
    const { horizon, fetchImpl } = client(json(200, account));

    await expect(horizon.getAccount(ACCOUNT)).resolves.toEqual(account);
    expect(fetchImpl).toHaveBeenCalledWith(`${HORIZON}/accounts/${ACCOUNT}`, expect.anything());
  });

  test('submitTransaction posts the envelope as a form-encoded tx field', async () => {
    const { horizon, fetchImpl } = client(json(200, tx('abc', '1')));

    await expect(horizon.submitTransaction('AAAA+/=')).resolves.toMatchObject({ hash: 'abc', successful: true });
    const [url, init] = (fetchImpl.mock.calls as unknown as [string, RequestInit][])[0];
    expect(url).toBe(`${HORIZON}/transactions`);
    expect(init.method).toBe('POST');
    expect(String(init.body)).toBe('tx=AAAA%2B%2F%3D');
  });

  test('a failed submission exposes the result codes', async () => {
    const { horizon } = client(
      json(400, {
        type: 'https://stellar.org/horizon-errors/transaction_failed',
        title: 'Transaction Failed',
        status: 400,
        detail: 'The transaction failed when submitted to the stellar network.',
        extras: { result_codes: { transaction: 'tx_failed', operations: ['op_underfunded'] } },
      }),
    );

    const err = await horizon.submitTransaction('AAAA').catch((e) => e);
    expect(err).toBeInstanceOf(HorizonError);
    expect(err.status).toBe(400);
    expect(err.title).toBe('Transaction Failed');
    expect(err.resultCodes).toEqual({ transaction: 'tx_failed', operations: ['op_underfunded'] });
  });

  test('rate limiting and non-JSON errors keep the HTTP status', async () => {
    const { horizon } = client(new Response('slow down', { status: 429, statusText: 'Too Many Requests' }));

    await expect(horizon.getAccount(ACCOUNT)).rejects.toMatchObject({ status: 429, title: 'Too Many Requests' });
  });

  test('network failures become status 0 errors', async () => {
    const { horizon } = client(new Error('ECONNRESET'));

    await expect(horizon.getAccount(ACCOUNT)).rejects.toMatchObject({ status: 0, detail: 'ECONNRESET' });
  });

  test('trackTransaction polls through 404s until the transaction is ledgered', async () => {
    const { horizon, fetchImpl } = client(
      json(404, { title: 'Resource Missing', status: 404 }),
      json(404, { title: 'Resource Missing', status: 404 }),
      json(200, tx('abc', '7')),
    );

    await expect(horizon.trackTransaction('abc', { intervalMs: 1, timeoutMs: 1_000 })).resolves.toMatchObject({
      hash: 'abc',
      ledger: 100,
    });
    expect(fetchImpl).toHaveBeenCalledTimes(3);
  });

  test('trackTransaction times out with a 408 error', async () => {
    const { horizon } = client(...Array.from({ length: 5 }, () => json(404, { title: 'Resource Missing' })));

    await expect(horizon.trackTransaction('abc', { intervalMs: 5, timeoutMs: 8 })).rejects.toMatchObject({
      status: 408,
    });
  });

  test('trackTransaction retries transient failures until the transaction is ledgered', async () => {
    const { horizon, fetchImpl } = client(
      new Error('ECONNRESET'),
      json(429, { title: 'Too Many Requests' }),
      json(503, { title: 'Service Unavailable' }),
      json(200, tx('abc', '7')),
    );

    await expect(horizon.trackTransaction('abc', { intervalMs: 1, timeoutMs: 1_000 })).resolves.toMatchObject({ hash: 'abc' });
    expect(fetchImpl).toHaveBeenCalledTimes(4);
  });

  test('trackTransaction stops on client errors', async () => {
    const { horizon } = client(json(400, { title: 'Bad Request', detail: 'invalid hash' }));

    await expect(horizon.trackTransaction('abc', { intervalMs: 1 })).rejects.toMatchObject({ status: 400 });
  });
});

describe('HorizonClient.monitorAccount', () => {
  test('streams transactions from SSE events and skips the hello message', async () => {
    const body =
      'retry: 1000\nevent: open\ndata: "hello"\n\n' +
      `id: 1\r\ndata: ${JSON.stringify(tx('one', '1'))}\r\n\r\n` +
      `id: 2\ndata: ${JSON.stringify(tx('two', '2'))}\n\n`;
    const { horizon, fetchImpl } = client(new Response(body, { status: 200 }));

    const seen: string[] = [];
    const handle = horizon.monitorAccount(ACCOUNT, (record) => {
      seen.push(record.hash);
      if (seen.length === 2) handle.close();
    });
    await handle.done;

    expect(seen).toEqual(['one', 'two']);
    expect(fetchImpl).toHaveBeenCalledWith(
      `${HORIZON}/accounts/${ACCOUNT}/transactions?cursor=now`,
      expect.objectContaining({ headers: { Accept: 'text/event-stream' } }),
    );
  });

  test('reconnects from the last paging token when Horizon closes the stream', async () => {
    const { horizon, fetchImpl } = client(
      new Response(`data: ${JSON.stringify(tx('one', '11'))}\n\n`, { status: 200 }),
      new Response(`data: ${JSON.stringify(tx('two', '12'))}\n\n`, { status: 200 }),
    );

    const seen: string[] = [];
    const handle = horizon.monitorAccount(
      ACCOUNT,
      (record) => {
        seen.push(record.hash);
        if (seen.length === 2) handle.close();
      },
      { cursor: '10' },
    );
    await handle.done;

    expect(seen).toEqual(['one', 'two']);
    expect((fetchImpl.mock.calls as unknown[][]).map((call) => call[0])).toEqual([
      `${HORIZON}/accounts/${ACCOUNT}/transactions?cursor=10`,
      `${HORIZON}/accounts/${ACCOUNT}/transactions?cursor=11`,
    ]);
  });

  test('reports HTTP errors through onError', async () => {
    const { horizon } = client(json(404, { title: 'Resource Missing', status: 404 }));

    const onError = jest.fn();
    await horizon.monitorAccount(ACCOUNT, jest.fn(), { onError }).done;

    expect(onError).toHaveBeenCalledWith(expect.objectContaining({ status: 404, title: 'Resource Missing' }));
  });
});
