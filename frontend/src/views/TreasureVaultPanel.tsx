import React, { useId } from 'react';
import { colors } from '../a11y';
import { useI18n } from '../i18n';

/** Mirrors `treasure_vault::LootRarity`. */
export type VaultLootRarity = 'Common' | 'Rare' | 'Epic' | 'Legendary';
/** Mirrors `treasure_vault::SpecialReward`. */
export type VaultSpecialReward = 'None' | 'StarChart' | 'AncientRelic';

export interface VaultPanelItem {
  id: number;
  /** Locked principal. */
  amount: number;
  /** End of the lock period in milliseconds since the epoch. */
  lockUntil: number;
  discovered: boolean;
  /** Present once discovered. */
  rarity?: VaultLootRarity;
  lootAmount?: number;
  special?: VaultSpecialReward;
  lootClaimed: boolean;
}

export interface TreasureVaultPanelProps {
  vaults: VaultPanelItem[];
  /** Unspent vault keys (`get_vault_key_balance`). */
  keyBalance: number;
  /** Reference time (ms) for lock checks; defaults to now. */
  now?: number;
  /** Vault whose transaction is in flight; its buttons show as busy. */
  pendingVaultId?: number;
  onDiscover?: (vaultId: number) => void;
  onClaim?: (vaultId: number) => void;
}

const rarityStyle: Record<VaultLootRarity, { color: string; glow: string }> = {
  Common: { color: colors.textMuted, glow: 'none' },
  Rare: { color: colors.accent, glow: '0 0 12px rgba(56, 189, 248, 0.25)' },
  Epic: { color: colors.epic, glow: '0 0 16px rgba(192, 132, 252, 0.35)' },
  Legendary: { color: colors.warning, glow: '0 0 22px rgba(251, 191, 36, 0.45)' },
};

const buttonStyle = (background: string, busy: boolean): React.CSSProperties => ({
  backgroundColor: background,
  color: colors.textOnAccent,
  border: 'none',
  borderRadius: 8,
  padding: '8px 14px',
  fontWeight: 700,
  cursor: busy ? 'progress' : 'pointer',
  opacity: busy ? 0.7 : 1,
});

export const TreasureVaultPanel: React.FC<TreasureVaultPanelProps> = ({
  vaults,
  keyBalance,
  now = Date.now(),
  pendingVaultId,
  onDiscover,
  onClaim,
}) => {
  const { t, formatNumber, formatDate } = useI18n();
  const titleId = useId();
  const needKeyId = useId();

  return (
    <section
      aria-labelledby={titleId}
      style={{
        backgroundColor: colors.bgSurface,
        border: `1px solid ${colors.border}`,
        borderRadius: 12,
        padding: 20,
        color: colors.textPrimary,
      }}
    >
      <div
        style={{
          display: 'flex',
          justifyContent: 'space-between',
          alignItems: 'baseline',
          flexWrap: 'wrap',
          gap: 8,
        }}
      >
        <h2 id={titleId} style={{ margin: 0, fontSize: '1.25rem' }}>
          {t('vault.title')}
        </h2>
        {/* Polite live region: announces when keys are forged or spent. */}
        <p role="status" style={{ margin: 0, color: colors.warning, fontWeight: 700 }}>
          <span aria-hidden="true">🗝 </span>
          {t('vault.keys', { count: keyBalance })}
        </p>
      </div>
      <p style={{ color: colors.textMuted, marginTop: 4 }}>{t('vault.description')}</p>
      <p id={needKeyId} hidden>
        {t('vault.needKey')}
      </p>

      {vaults.length === 0 ? (
        <p style={{ color: colors.textSecondary }}>{t('vault.empty')}</p>
      ) : (
        <ul
          aria-label={t('vault.vaultsLabel')}
          style={{
            listStyle: 'none',
            padding: 0,
            margin: 0,
            display: 'grid',
            gridTemplateColumns: 'repeat(auto-fill, minmax(220px, 1fr))',
            gap: 12,
          }}
        >
          {vaults.map((vault) => {
            const locked = now < vault.lockUntil;
            const busy = pendingVaultId === vault.id;
            const rarity = vault.discovered && vault.rarity ? rarityStyle[vault.rarity] : undefined;
            const headingId = `${titleId}-vault-${vault.id}`;

            let body: React.ReactNode;
            if (locked) {
              body = (
                <p style={{ margin: 0, color: colors.textMuted }}>
                  <span aria-hidden="true">🔒 </span>
                  {t('vault.lockedUntil', { time: formatDate(vault.lockUntil) })}
                </p>
              );
            } else if (!vault.discovered) {
              body = (
                <>
                  <p style={{ margin: 0, color: colors.textMuted }}>{t('vault.undiscovered')}</p>
                  <button
                    type="button"
                    aria-busy={busy}
                    aria-label={t('vault.discoverVault', { id: vault.id })}
                    onClick={() => !busy && onDiscover?.(vault.id)}
                    style={buttonStyle(colors.primaryFill, busy)}
                  >
                    {t('vault.discover')}
                  </button>
                </>
              );
            } else {
              const noKey = keyBalance === 0;
              const special = vault.special && vault.special !== 'None' ? vault.special : undefined;
              body = (
                <>
                  <p style={{ margin: 0, fontWeight: 700, color: rarity?.color }}>
                    {vault.rarity ? t(`rarity.${vault.rarity}`) : null}
                  </p>
                  <p style={{ margin: 0 }}>
                    {t('vault.loot', { amount: formatNumber(vault.lootAmount ?? 0) })}
                  </p>
                  {special && (
                    <p style={{ margin: 0, color: colors.warning }}>
                      <span aria-hidden="true">✦ </span>
                      {t(`vault.special.${special}`)}
                    </p>
                  )}
                  {vault.lootClaimed ? (
                    <p style={{ margin: 0, color: colors.success, fontWeight: 700 }}>
                      <span aria-hidden="true">✓ </span>
                      {t('vault.claimed')}
                    </p>
                  ) : (
                    // aria-disabled (not disabled) keeps the button focusable so
                    // screen-reader users hear why it cannot be used yet.
                    <button
                      type="button"
                      aria-busy={busy}
                      aria-disabled={noKey || undefined}
                      aria-describedby={noKey ? needKeyId : undefined}
                      aria-label={t('vault.claimVault', { id: vault.id })}
                      onClick={() => !noKey && !busy && onClaim?.(vault.id)}
                      style={{
                        ...buttonStyle(colors.successFill, busy),
                        cursor: noKey ? 'not-allowed' : busy ? 'progress' : 'pointer',
                        opacity: noKey ? 0.6 : busy ? 0.7 : 1,
                      }}
                    >
                      {t('vault.claim')}
                    </button>
                  )}
                </>
              );
            }

            return (
              <li
                key={vault.id}
                aria-labelledby={headingId}
                style={{
                  backgroundColor: colors.bgRaised,
                  border: `2px solid ${rarity?.color ?? colors.border}`,
                  boxShadow: rarity?.glow,
                  borderRadius: 12,
                  padding: 16,
                  display: 'flex',
                  flexDirection: 'column',
                  gap: 8,
                }}
              >
                <h3 id={headingId} style={{ margin: 0, fontSize: '1rem' }}>
                  {t('vault.vault', { id: vault.id })}
                </h3>
                <p style={{ margin: 0, color: colors.textSecondary, fontSize: '0.85rem' }}>
                  {t('vault.principal', { amount: formatNumber(vault.amount) })}
                </p>
                {body}
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
};
