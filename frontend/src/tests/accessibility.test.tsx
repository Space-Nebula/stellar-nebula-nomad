/**
 * WCAG 2.1 AA tests (Issue #698).
 *
 * - axe-core audits every shared component and the oracle/vault views, in
 *   English and in Arabic (RTL).
 * - jsdom has no layout engine, so axe cannot measure colour contrast; the
 *   palette is checked explicitly against the AA ratios instead.
 * - Keyboard and focus behaviour is exercised directly.
 */
import React, { useState } from 'react';
import '@testing-library/jest-dom';
import { act, fireEvent, render, screen } from '@testing-library/react';
import { axe, toHaveNoViolations } from 'jest-axe';
import { colors, contrastRatio } from '../a11y';
import { i18n } from '../i18n';
import {
  AchievementBadge,
  AddressInput,
  BondButton,
  ConfirmDialog,
  DashboardLayout,
  DurationPicker,
  ErrorBoundary,
  LeaderboardTable,
  LoadingSkeleton,
  MintButton,
  ModalWrapper,
  NebulaVisualization,
  RaritySelector,
  ResourceAmount,
  ResourceDisplay,
  ShipCard,
  TabContainer,
  ToastNotification,
  TradeButton,
  UpgradeButton,
} from '../components';
import { MarketOracleDashboard } from '../views/MarketOracleDashboard';
import { TreasureVaultPanel } from '../views/TreasureVaultPanel';

expect.extend(toHaveNoViolations);

afterEach(async () => {
  await act(async () => {
    await i18n.changeLanguage('en');
  });
});

const noop = () => undefined;
const NOW = Date.UTC(2026, 10, 20);

const Broken: React.FC = () => {
  throw new Error('boom');
};

const gallery: [string, React.ReactElement][] = [
  [
    'ShipCard',
    <ShipCard
      key="ship"
      id="s1"
      name="Vanguard"
      shipClass="Scout"
      rarity="Epic"
      level={3}
      stats={{ speed: 10, hull: 20, scanPower: 30 }}
      onSelect={noop}
    />,
  ],
  [
    'ResourceDisplay',
    <ResourceDisplay key="res" name="Ore" amount={950} maxCapacity={1000} change24h={-3} />,
  ],
  [
    'NebulaVisualization',
    <NebulaVisualization key="neb" sectorName="Orion" density={42} anomalyDetected />,
  ],
  [
    'LeaderboardTable',
    <LeaderboardTable
      key="lb"
      entries={[
        { rank: 1, playerId: 'GABCDEFGHIJKLMNOP', score: 1200, guild: 'Nova' },
        { rank: 2, playerId: 'GZYXWVUTSRQPONMLK', score: 900 },
      ]}
      currentPlayerId="GZYXWVUTSRQPONMLK"
    />,
  ],
  [
    'AchievementBadge',
    <AchievementBadge key="ach" id="a" title="Explorer" description="Visit 10 sectors" unlocked={false} />,
  ],
  ['ResourceAmount', <ResourceAmount key="amt" value={5} onChange={noop} error="Too much" />],
  ['AddressInput', <AddressInput key="addr" value="" onChange={noop} />],
  ['DurationPicker', <DurationPicker key="dur" valueHours={24} onChange={noop} />],
  ['RaritySelector', <RaritySelector key="rar" selected="Rare" onChange={noop} />],
  ['MintButton', <MintButton key="mint" onMint={noop} isLoading />],
  ['TradeButton', <TradeButton key="trade" onTrade={noop} tradeType="sell" price="5" />],
  ['UpgradeButton', <UpgradeButton key="up" onUpgrade={noop} targetLevel={4} />],
  ['BondButton', <BondButton key="bond" onBond={noop} stakeAmount="10 XLM" guildName="Nova" />],
  ['LoadingSkeleton', <LoadingSkeleton key="load" count={3} />],
  [
    'ToastNotification',
    <ToastNotification key="toast" id="t" title="Saved" message="Ship saved" onDismiss={noop} />,
  ],
  [
    'ErrorBoundary',
    <ErrorBoundary key="err">
      <Broken />
    </ErrorBoundary>,
  ],
  [
    'TabContainer',
    <TabContainer
      key="tabs"
      ariaLabel="Ship sections"
      tabs={[
        { id: 'a', label: 'Stats', content: <p>Stats</p> },
        { id: 'b', label: 'Crew', content: <p>Crew</p> },
      ]}
    />,
  ],
  [
    'ConfirmDialog',
    <ConfirmDialog
      key="confirm"
      isOpen
      title="Dismantle ship?"
      message="This cannot be undone."
      onConfirm={noop}
      onCancel={noop}
      isDestructive
    />,
  ],
  [
    'DashboardLayout',
    <DashboardLayout key="layout" header={<h1>Nomad</h1>} sidebar={<a href="#ships">Ships</a>}>
      <h2>Fleet</h2>
    </DashboardLayout>,
  ],
  [
    'MarketOracleDashboard',
    <MarketOracleDashboard
      key="oracle"
      now={NOW}
      prices={[
        { resource: 'ore', price: 12, sourceCount: 3, updatedAt: NOW - 60_000, status: 'fresh' },
        { resource: 'gas', price: 4, sourceCount: 2, updatedAt: NOW - 60_000, status: 'rejected' },
      ]}
    />,
  ],
  [
    'TreasureVaultPanel',
    <TreasureVaultPanel
      key="vault"
      now={NOW}
      keyBalance={0}
      vaults={[
        { id: 1, amount: 100, lockUntil: NOW + 1000, discovered: false, lootClaimed: false },
        { id: 2, amount: 100, lockUntil: NOW - 1000, discovered: false, lootClaimed: false },
        {
          id: 3,
          amount: 100,
          lockUntil: NOW - 1000,
          discovered: true,
          rarity: 'Epic',
          lootAmount: 120,
          special: 'StarChart',
          lootClaimed: false,
        },
      ]}
    />,
  ],
];

describe('axe audit', () => {
  // The ErrorBoundary case logs the thrown error; keep test output readable.
  let consoleError: jest.SpyInstance;
  beforeAll(() => {
    consoleError = jest.spyOn(console, 'error').mockImplementation(() => undefined);
  });
  afterAll(() => consoleError.mockRestore());

  test.each(gallery)('%s has no violations (en)', async (_name, element) => {
    const { container } = render(element);
    expect(await axe(container)).toHaveNoViolations();
  });

  test.each(gallery)('%s has no violations (ar, RTL)', async (_name, element) => {
    await act(async () => {
      await i18n.changeLanguage('ar');
    });
    const { container } = render(element);
    expect(await axe(container)).toHaveNoViolations();
  });
});

describe('colour contrast (WCAG 1.4.3 / 1.4.11)', () => {
  const TEXT = 4.5;
  const NON_TEXT = 3;
  const surfaces = [colors.bgApp, colors.bgSurface, colors.bgRaised];

  test.each([
    ['textPrimary', colors.textPrimary],
    ['textSecondary', colors.textSecondary],
    ['textMuted', colors.textMuted],
    ['accent', colors.accent],
    ['success', colors.success],
    ['danger', colors.danger],
    ['warning', colors.warning],
    ['epic', colors.epic],
  ])('%s text is readable on every surface', (_name, fg) => {
    for (const bg of surfaces) {
      expect(contrastRatio(fg, bg)).toBeGreaterThanOrEqual(TEXT);
    }
  });

  test.each([
    ['primaryFill', colors.textOnAccent, colors.primaryFill],
    ['secondaryFill', colors.textOnAccent, colors.secondaryFill],
    ['successFill', colors.textOnAccent, colors.successFill],
    ['dangerFill', colors.textOnAccent, colors.dangerFill],
    ['bondFill', colors.textOnAccent, colors.bondFill],
    ['selectedFill', colors.textOnAccent, colors.selectedFill],
    ['warningFill', colors.textOnWarning, colors.warningFill],
    ['secondary button', colors.textPrimary, colors.border],
  ])('button text on %s meets AA', (_name, fg, bg) => {
    expect(contrastRatio(fg, bg)).toBeGreaterThanOrEqual(TEXT);
  });

  test('input borders and the focus ring are visible (3:1)', () => {
    for (const bg of surfaces) {
      expect(contrastRatio(colors.textMuted, bg)).toBeGreaterThanOrEqual(NON_TEXT);
      expect(contrastRatio(colors.focus, bg)).toBeGreaterThanOrEqual(NON_TEXT);
    }
  });

  test('the old failing pairs really did fail', () => {
    // Regression guard documenting why the palette changed.
    expect(contrastRatio('#FFFFFF', '#3B82F6')).toBeLessThan(TEXT);
    expect(contrastRatio('#FFFFFF', '#10B981')).toBeLessThan(TEXT);
    expect(contrastRatio('#FFFFFF', '#F59E0B')).toBeLessThan(TEXT);
    expect(contrastRatio('#64748B', '#1E293B')).toBeLessThan(TEXT);
  });
});

describe('keyboard and focus management', () => {
  const DialogHarness: React.FC = () => {
    const [open, setOpen] = useState(false);
    return (
      <>
        <button type="button" onClick={() => setOpen(true)}>
          Open
        </button>
        <ModalWrapper isOpen={open} onClose={() => setOpen(false)} title="Settings">
          <button type="button">First action</button>
          <button type="button">Last action</button>
        </ModalWrapper>
      </>
    );
  };

  test('dialog takes focus, traps Tab, closes on Escape and restores focus', () => {
    render(<DialogHarness />);
    const opener = screen.getByRole('button', { name: 'Open' });
    opener.focus();
    fireEvent.click(opener);

    const dialog = screen.getByRole('dialog', { name: 'Settings' });
    expect(dialog).toHaveAttribute('aria-modal', 'true');
    const close = screen.getByRole('button', { name: 'Close' });
    expect(close).toHaveFocus();

    const last = screen.getByRole('button', { name: 'Last action' });
    last.focus();
    fireEvent.keyDown(document, { key: 'Tab' });
    expect(close).toHaveFocus();
    fireEvent.keyDown(document, { key: 'Tab', shiftKey: true });
    expect(last).toHaveFocus();

    fireEvent.keyDown(document, { key: 'Escape' });
    expect(screen.queryByRole('dialog')).toBeNull();
    expect(opener).toHaveFocus();
  });

  const tabs = [
    { id: 'a', label: 'Alpha', content: <p>A</p> },
    { id: 'b', label: 'Beta', content: <p>B</p> },
    { id: 'c', label: 'Gamma', content: <p>C</p> },
  ];

  test('tabs: roving tabindex, arrows, Home and End', () => {
    render(<TabContainer tabs={tabs} ariaLabel="Sections" />);
    const [alpha, beta, gamma] = screen.getAllByRole('tab');
    expect(alpha).toHaveAttribute('tabindex', '0');
    expect(beta).toHaveAttribute('tabindex', '-1');

    alpha.focus();
    fireEvent.keyDown(alpha, { key: 'ArrowRight' });
    expect(beta).toHaveFocus();
    expect(beta).toHaveAttribute('aria-selected', 'true');
    expect(screen.getByRole('tabpanel')).toHaveAccessibleName('Beta');

    fireEvent.keyDown(beta, { key: 'End' });
    expect(gamma).toHaveFocus();
    fireEvent.keyDown(gamma, { key: 'ArrowRight' });
    expect(alpha).toHaveFocus(); // wraps
    fireEvent.keyDown(alpha, { key: 'ArrowLeft' });
    expect(gamma).toHaveFocus();
    fireEvent.keyDown(gamma, { key: 'Home' });
    expect(alpha).toHaveFocus();
  });

  test('tabs: arrow keys follow reading direction in RTL', async () => {
    await act(async () => {
      await i18n.changeLanguage('ar');
    });
    render(<TabContainer tabs={tabs} />);
    const [alpha, beta] = screen.getAllByRole('tab');
    alpha.focus();
    fireEvent.keyDown(alpha, { key: 'ArrowLeft' });
    expect(beta).toHaveFocus();
  });

  test('two tab containers on one page do not share ids', () => {
    render(
      <>
        <TabContainer tabs={tabs} />
        <TabContainer tabs={tabs} />
      </>
    );
    const ids = screen.getAllByRole('tab').map((t) => t.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  test('ship cards expose a real, labelled select button', () => {
    const onSelect = jest.fn();
    render(
      <ShipCard
        id="s1"
        name="Vanguard"
        shipClass="Scout"
        rarity="Rare"
        level={1}
        stats={{ speed: 1, hull: 1, scanPower: 1 }}
        onSelect={onSelect}
      />
    );
    fireEvent.click(screen.getByRole('button', { name: 'Select Vanguard' }));
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(onSelect).toHaveBeenCalledWith('s1');
  });

  test('layout has a skip link to the focusable main landmark', () => {
    render(
      <DashboardLayout header={<span>Header</span>} sidebar={<span>Menu</span>}>
        <p>Body</p>
      </DashboardLayout>
    );
    const skip = screen.getByRole('link', { name: 'Skip to main content' });
    expect(skip).toHaveAttribute('href', '#main-content');
    const main = screen.getByRole('main');
    expect(main).toHaveAttribute('id', 'main-content');
    expect(main).toHaveAttribute('tabindex', '-1');
    expect(screen.getByRole('navigation', { name: 'Primary navigation' })).toBeInTheDocument();
  });
});

describe('screen reader semantics', () => {
  test('toggle groups are labelled and expose pressed state', () => {
    render(<DurationPicker valueHours={24} onChange={noop} />);
    expect(screen.getByRole('group', { name: 'Expedition Duration' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: '1 day' })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.getByRole('button', { name: '4 hours' })).toHaveAttribute('aria-pressed', 'false');
  });

  test('validation errors are announced and linked to the field', () => {
    render(<AddressInput value="GBAD" onChange={noop} />);
    const input = screen.getByLabelText('Stellar Account Address');
    fireEvent.blur(input);
    expect(input).toHaveAttribute('aria-invalid', 'true');
    expect(input).toHaveAccessibleDescription(/Invalid Stellar public key format/);
    expect(screen.getByRole('alert')).toBeInTheDocument();
  });

  test('two inputs on one page get distinct ids', () => {
    render(
      <>
        <AddressInput value="" onChange={noop} label="From" />
        <AddressInput value="" onChange={noop} label="To" />
      </>
    );
    expect(screen.getByLabelText('From').id).not.toBe(screen.getByLabelText('To').id);
  });

  test('only urgent toasts interrupt', () => {
    const { rerender } = render(
      <ToastNotification id="t" title="Saved" message="ok" type="success" onDismiss={noop} />
    );
    expect(screen.getByRole('status')).toHaveTextContent('Saved');
    rerender(<ToastNotification id="t" title="Failed" message="no" type="error" onDismiss={noop} />);
    expect(screen.getByRole('alert')).toHaveTextContent('Failed');
  });

  test('capacity is exposed as a progress bar', () => {
    render(<ResourceDisplay name="Ore" amount={500} maxCapacity={1000} />);
    const bar = screen.getByRole('progressbar', { name: 'Ore capacity used' });
    expect(bar).toHaveAttribute('aria-valuenow', '50');
  });

  test('the nebula image label includes the anomaly', () => {
    render(<NebulaVisualization sectorName="Orion" density={42} anomalyDetected />);
    expect(screen.getByRole('img')).toHaveAccessibleName(
      'Nebula sector: Orion, density 42%, anomaly detected'
    );
  });

  test('the current player row is marked without relying on colour', () => {
    render(
      <LeaderboardTable
        entries={[{ rank: 1, playerId: 'GABCDEFGHIJK', score: 5 }]}
        currentPlayerId="gabcdefghijk"
      />
    );
    const row = screen.getAllByRole('row')[1];
    expect(row).toHaveAttribute('aria-current', 'true');
    expect(row).toHaveTextContent('(you)');
    expect(screen.getByRole('table', { name: 'Leaderboard Rankings' })).toBeInTheDocument();
  });

  test('loading state is announced politely', () => {
    render(<LoadingSkeleton />);
    const status = screen.getByRole('status');
    expect(status).toHaveAttribute('aria-busy', 'true');
    expect(status).toHaveTextContent('Loading...');
  });
});
