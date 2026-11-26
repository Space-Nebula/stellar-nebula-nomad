/**
 * Oracle dashboard (#654) and treasure vault panel (#655) UI tests.
 */
import React from 'react';
import '@testing-library/jest-dom';
import { act, fireEvent, render, screen, within } from '@testing-library/react';
import { i18n, I18nProvider } from '../i18n';
import { MarketOracleDashboard, OraclePrice } from '../views/MarketOracleDashboard';
import { TreasureVaultPanel, VaultPanelItem } from '../views/TreasureVaultPanel';

const NOW = Date.UTC(2026, 10, 20, 12, 0, 0);
const HOUR = 3600 * 1000;

afterEach(async () => {
  await act(async () => {
    await i18n.changeLanguage('en');
  });
});

describe('MarketOracleDashboard', () => {
  const prices: OraclePrice[] = [
    { resource: 'ore', price: 1234.5, sourceCount: 4, updatedAt: NOW - 2 * HOUR, status: 'fresh' },
    { resource: 'gas', price: 87, sourceCount: 1, updatedAt: NOW - 30 * HOUR, status: 'stale' },
    { resource: 'dust', price: 10, sourceCount: 3, updatedAt: NOW - HOUR, status: 'rejected' },
  ];

  test('renders one row per resource with price, sources, age and status text', () => {
    render(<MarketOracleDashboard prices={prices} now={NOW} />);
    const table = screen.getByRole('table', { name: 'Resource prices' });
    const ore = within(table).getByRole('row', { name: /ore/ });
    expect(ore).toHaveTextContent('1,234.5');
    expect(ore).toHaveTextContent('4 agreeing sources');
    expect(ore).toHaveTextContent('2 hours ago');
    expect(ore).toHaveTextContent('Validated');

    expect(within(table).getByRole('row', { name: /gas/ })).toHaveTextContent('1 agreeing source');
    expect(within(table).getByRole('row', { name: /gas/ })).toHaveTextContent('Stale');
    const dust = within(table).getByRole('row', { name: /dust/ });
    expect(dust).toHaveTextContent('Rejected');
    expect(dust).toHaveTextContent(/last validated price/);
  });

  test('is a labelled region with an empty state', () => {
    render(<MarketOracleDashboard prices={[]} now={NOW} />);
    expect(screen.getByRole('region', { name: 'Market Oracle' })).toBeInTheDocument();
    expect(screen.getByText('No prices published yet.')).toBeInTheDocument();
  });

  test('formats numbers and relative time for the active locale', async () => {
    render(
      <I18nProvider>
        <MarketOracleDashboard prices={prices.slice(0, 1)} now={NOW} />
      </I18nProvider>
    );
    await act(async () => {
      await i18n.changeLanguage('es');
    });
    const row = screen.getByRole('row', { name: /ore/ });
    expect(row).toHaveTextContent('1234,5');
    expect(row).toHaveTextContent('hace 2 horas');
    expect(row).toHaveTextContent('Validado');
  });
});

describe('TreasureVaultPanel', () => {
  const base: VaultPanelItem = {
    id: 7,
    amount: 1000,
    lockUntil: NOW - HOUR,
    discovered: false,
    lootClaimed: false,
  };

  test('locked vaults show the unlock time and no actions', () => {
    render(
      <TreasureVaultPanel vaults={[{ ...base, lockUntil: NOW + HOUR }]} keyBalance={0} now={NOW} />
    );
    const card = screen.getByRole('listitem', { name: 'Vault #7' });
    expect(card).toHaveTextContent(/Locked until/);
    expect(within(card).queryByRole('button')).toBeNull();
  });

  test('unlocked vaults can be discovered', () => {
    const onDiscover = jest.fn();
    render(<TreasureVaultPanel vaults={[base]} keyBalance={0} now={NOW} onDiscover={onDiscover} />);
    fireEvent.click(screen.getByRole('button', { name: 'Discover vault #7' }));
    expect(onDiscover).toHaveBeenCalledWith(7);
  });

  test('discovered loot shows rarity, amount and special reward; claim needs a key', () => {
    const onClaim = jest.fn();
    const legendary: VaultPanelItem = {
      ...base,
      discovered: true,
      rarity: 'Legendary',
      lootAmount: 3000,
      special: 'AncientRelic',
    };
    const { rerender } = render(
      <TreasureVaultPanel vaults={[legendary]} keyBalance={0} now={NOW} onClaim={onClaim} />
    );
    const card = screen.getByRole('listitem', { name: 'Vault #7' });
    expect(card).toHaveTextContent('Legendary');
    expect(card).toHaveTextContent('Loot: 3,000');
    expect(card).toHaveTextContent('Ancient Relic');

    // No key: the button stays focusable but explains why it does nothing.
    const claim = screen.getByRole('button', { name: 'Claim loot from vault #7' });
    expect(claim).toHaveAttribute('aria-disabled', 'true');
    expect(claim).toHaveAccessibleDescription('You need a vault key to claim this loot.');
    fireEvent.click(claim);
    expect(onClaim).not.toHaveBeenCalled();

    rerender(<TreasureVaultPanel vaults={[legendary]} keyBalance={1} now={NOW} onClaim={onClaim} />);
    fireEvent.click(screen.getByRole('button', { name: 'Claim loot from vault #7' }));
    expect(onClaim).toHaveBeenCalledWith(7);
  });

  test('claimed loot is reported and the key balance is a live status', () => {
    render(
      <TreasureVaultPanel
        vaults={[{ ...base, discovered: true, rarity: 'Rare', lootAmount: 500, lootClaimed: true }]}
        keyBalance={2}
        now={NOW}
      />
    );
    expect(screen.getByText('Loot claimed')).toBeInTheDocument();
    expect(screen.getByRole('status')).toHaveTextContent('2 vault keys');
  });

  test('busy vaults ignore repeated clicks', () => {
    const onDiscover = jest.fn();
    render(
      <TreasureVaultPanel
        vaults={[base]}
        keyBalance={0}
        now={NOW}
        pendingVaultId={7}
        onDiscover={onDiscover}
      />
    );
    const button = screen.getByRole('button', { name: 'Discover vault #7' });
    expect(button).toHaveAttribute('aria-busy', 'true');
    fireEvent.click(button);
    expect(onDiscover).not.toHaveBeenCalled();
  });

  test('uses Arabic plural forms', async () => {
    render(<TreasureVaultPanel vaults={[]} keyBalance={2} now={NOW} />);
    await act(async () => {
      await i18n.changeLanguage('ar');
    });
    expect(screen.getByRole('status')).toHaveTextContent('مفتاحا خزنة');
  });
});
