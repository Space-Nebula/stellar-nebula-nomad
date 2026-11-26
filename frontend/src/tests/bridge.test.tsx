import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { Bridge } from '../views/Bridge';
import { amountError, bridgeFee, isValidEthAddress, MAX_BRIDGE_AMOUNT, MAX_BRIDGE_FEE_BPS } from '../utils/bridge';

const RECIPIENT = '0x1111111111111111111111111111111111111111';

describe('bridge rule mirrors', () => {
  test('fee matches the contract (floor of amount * bps / 10_000)', () => {
    expect(bridgeFee(10_000, 30)).toBe(30);
    expect(bridgeFee(1_000_000, 30)).toBe(3_000);
    expect(bridgeFee(10, 30)).toBe(0);
    expect(bridgeFee(MAX_BRIDGE_AMOUNT, MAX_BRIDGE_FEE_BPS)).toBe(MAX_BRIDGE_AMOUNT / 100);
  });

  test('Ethereum addresses must be 20 bytes and non-zero', () => {
    expect(isValidEthAddress(RECIPIENT)).toBe(true);
    expect(isValidEthAddress('0xAbCdEf0123456789abcdef0123456789ABCDEF01')).toBe(true);
    expect(isValidEthAddress('0x' + '0'.repeat(40))).toBe(false);
    expect(isValidEthAddress('0x1234')).toBe(false);
    expect(isValidEthAddress('1111111111111111111111111111111111111111')).toBe(false);
  });

  test('amount checks mirror InvalidAmount and AmountExceeded', () => {
    expect(amountError(100, 1_000, 500)).toBeNull();
    expect(amountError(0, 1_000, 500)).toMatch(/above zero/);
    expect(amountError(1.5, 1_000, 500)).toMatch(/whole amount/);
    expect(amountError(2_000, 1_000, 5_000)).toMatch(/limit/);
    expect(amountError(600, 1_000, 500)).toMatch(/balance/);
  });
});

describe('Bridge view', () => {
  const assets = [
    { symbol: 'XLM', chainId: 1, enabled: true, balance: 1_000_000 },
    { symbol: 'OLD', chainId: 1, enabled: false, balance: 5 },
  ];

  function renderBridge(overrides: Partial<React.ComponentProps<typeof Bridge>> = {}) {
    const onBridge = jest.fn();
    const onCancel = jest.fn();
    render(
      <Bridge
        assets={assets}
        feeBps={30}
        maxAmount={MAX_BRIDGE_AMOUNT}
        paused={false}
        requests={[]}
        onBridge={onBridge}
        onCancel={onCancel}
        {...overrides}
      />,
    );
    return { onBridge, onCancel };
  }

  test('previews the fee and submits a valid transfer', () => {
    const { onBridge } = renderBridge();
    expect(screen.queryByRole('option', { name: /OLD/ })).toBeNull();

    fireEvent.change(screen.getByLabelText(/Amount/), { target: { value: '10000' } });
    fireEvent.change(screen.getByLabelText(/Ethereum recipient/), { target: { value: RECIPIENT } });
    expect(screen.getByText(/Fee 30 \(0.3%\) · Received on Ethereum 9,970/)).toBeTruthy();

    fireEvent.click(screen.getByText('Bridge to Ethereum'));
    expect(onBridge).toHaveBeenCalledWith({ asset: 'XLM', chainId: 1, amount: 10_000, ethRecipient: RECIPIENT });
  });

  test('works when assets arrive after the first render', () => {
    const onBridge = jest.fn();
    const props = { feeBps: 30, maxAmount: MAX_BRIDGE_AMOUNT, paused: false, requests: [], onBridge, onCancel: jest.fn() };
    const { rerender } = render(<Bridge assets={[]} {...props} />);
    rerender(<Bridge assets={assets} {...props} />);

    fireEvent.change(screen.getByLabelText(/Amount/), { target: { value: '100' } });
    fireEvent.change(screen.getByLabelText(/Ethereum recipient/), { target: { value: RECIPIENT } });
    fireEvent.click(screen.getByText('Bridge to Ethereum'));
    expect(onBridge).toHaveBeenCalledWith({ asset: 'XLM', chainId: 1, amount: 100, ethRecipient: RECIPIENT });
  });

  test('blocks invalid input and a paused bridge', () => {
    const { onBridge } = renderBridge({ paused: true });
    expect(screen.getByRole('alert').textContent).toMatch(/paused/);

    fireEvent.change(screen.getByLabelText(/Amount/), { target: { value: '10000' } });
    fireEvent.change(screen.getByLabelText(/Ethereum recipient/), { target: { value: RECIPIENT } });
    const submit = screen.getByText('Bridge to Ethereum') as HTMLButtonElement;
    expect(submit.disabled).toBe(true);
    fireEvent.click(submit);
    expect(onBridge).not.toHaveBeenCalled();
  });

  test('explains a bad recipient address', () => {
    renderBridge();
    fireEvent.change(screen.getByLabelText(/Amount/), { target: { value: '10' } });
    fireEvent.change(screen.getByLabelText(/Ethereum recipient/), { target: { value: '0x123' } });
    expect(screen.getByRole('alert').textContent).toMatch(/Ethereum address/);
    expect((screen.getByText('Bridge to Ethereum') as HTMLButtonElement).disabled).toBe(true);
  });

  test('lists requests and offers cancel only for pending outbound ones', () => {
    const { onCancel } = renderBridge({
      requests: [
        { requestId: 3, asset: 'XLM', chainId: 1, amount: 500, fee: 1, direction: 'StellarToEth', status: 'Pending', ethAddress: RECIPIENT },
        { requestId: 2, asset: 'XLM', chainId: 1, amount: 700, fee: 2, direction: 'EthToStellar', status: 'Completed', ethAddress: RECIPIENT },
        { requestId: 1, asset: 'XLM', chainId: 1, amount: 900, fee: 2, direction: 'StellarToEth', status: 'Failed', ethAddress: RECIPIENT },
      ],
    });

    expect(screen.getByText(/#2 · From Ethereum · 700 XLM · Completed/)).toBeTruthy();
    expect(screen.getByText(/#1 · To Ethereum · 900 XLM · Failed \(refunded\)/)).toBeTruthy();
    const cancel = screen.getAllByText('Cancel');
    expect(cancel).toHaveLength(1);
    fireEvent.click(cancel[0]);
    expect(onCancel).toHaveBeenCalledWith(3);
  });
});
