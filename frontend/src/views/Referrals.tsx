/**
 * Referral program UI (Issue #701): share code, statistics, tier progress,
 * referee list with claims, and code redemption for new players.
 */
import React, { useId, useState } from 'react';

import { useI18n } from '../i18n';
import { themeColors as c } from '../theme/themes';
import {
  buildReferralLink,
  formatReferralCode,
  isValidReferralCode,
  nextTier,
  normalizeReferralCode,
  refereeStatus,
  referrerReward,
  tierForCount,
  tierProgress,
} from '../utils/referrals';

/** Mirrors `referral_system::ReferralStats`. */
export interface ReferralStatsView {
  code?: string;
  totalReferrals: number;
  convertedReferrals: number;
  claimedReferrals: number;
  pendingReferrals: number;
  claimableReferrals: number;
  totalEssenceEarned: number;
  blocked: boolean;
}

export interface RefereeView {
  address: string;
  /** Registration time, ms since epoch. */
  registeredAt: number;
  firstScanDone: boolean;
  claimed: boolean;
}

export interface ReferralPanelProps {
  stats: ReferralStatsView;
  referees: RefereeView[];
  /** Base reward per claim (`get_reward_per_claim`). */
  baseReward: number;
  /** Welcome bonus for the new nomad (`get_referee_bonus`). */
  refereeBonus?: number;
  /** URL the share link points at; `?ref=CODE` is appended. */
  shareBaseUrl?: string;
  /** Referee address whose claim is in flight. */
  pendingClaim?: string;
  generating?: boolean;
  onGenerateCode?: () => void;
  onClaim?: (refereeAddress: string) => void;
  /** Shown to players who have not been referred yet. */
  onRedeemCode?: (code: string) => void;
  /** Clipboard writer; defaults to `navigator.clipboard.writeText`. */
  copyText?: (text: string) => Promise<void>;
}

const shortAddress = (a: string) => (a.length > 12 ? `${a.slice(0, 6)}…${a.slice(-4)}` : a);

const button = (fill: string, disabled = false): React.CSSProperties => ({
  backgroundColor: fill,
  color: c.textOnAccent,
  border: 'none',
  borderRadius: 8,
  padding: '8px 14px',
  fontWeight: 700,
  cursor: disabled ? 'not-allowed' : 'pointer',
  opacity: disabled ? 0.6 : 1,
});

export const ReferralPanel: React.FC<ReferralPanelProps> = ({
  stats,
  referees,
  baseReward,
  refereeBonus = 0,
  shareBaseUrl = 'https://stellar-nebula-nomad.app/join',
  pendingClaim,
  generating,
  onGenerateCode,
  onClaim,
  onRedeemCode,
  copyText,
}) => {
  const { t, formatNumber, formatDate } = useI18n();
  const titleId = useId();
  const codeInputId = useId();
  const codeErrorId = useId();
  const [copyStatus, setCopyStatus] = useState('');
  const [codeInput, setCodeInput] = useState('');
  const [codeError, setCodeError] = useState(false);

  const tier = tierForCount(stats.totalReferrals);
  const next = nextTier(stats.totalReferrals);
  const progress = Math.round(tierProgress(stats.totalReferrals) * 100);
  const link = stats.code ? buildReferralLink(shareBaseUrl, stats.code) : undefined;

  const copy = async () => {
    if (!link) return;
    try {
      await (copyText ?? ((text: string) => navigator.clipboard.writeText(text)))(link);
      setCopyStatus(t('referral.copied'));
    } catch {
      setCopyStatus(t('referral.copyFailed'));
    }
  };

  const statItems: [string, string][] = [
    [t('referral.stats.total'), formatNumber(stats.totalReferrals)],
    [t('referral.stats.pending'), formatNumber(stats.pendingReferrals)],
    [t('referral.stats.claimable'), formatNumber(stats.claimableReferrals)],
    [t('referral.stats.claimed'), formatNumber(stats.claimedReferrals)],
    [t('referral.stats.earned'), formatNumber(stats.totalEssenceEarned)],
  ];

  return (
    <section
      aria-labelledby={titleId}
      style={{
        backgroundColor: c.bgSurface,
        color: c.textPrimary,
        border: `1px solid ${c.border}`,
        borderRadius: 12,
        padding: 20,
        display: 'flex',
        flexDirection: 'column',
        gap: 16,
      }}
    >
      <div>
        <h2 id={titleId} style={{ margin: 0, fontSize: '1.25rem' }}>
          {t('referral.title')}
        </h2>
        <p style={{ margin: '4px 0 0', color: c.textMuted }}>
          {t('referral.description', {
            reward: formatNumber(referrerReward(baseReward, tier.multiplierBps)),
            bonus: formatNumber(refereeBonus),
          })}
        </p>
      </div>

      {stats.blocked && (
        <p role="alert" style={{ margin: 0, color: c.danger, fontWeight: 700 }}>
          {t('referral.blocked')}
        </p>
      )}

      {/* Code & share */}
      <div style={{ backgroundColor: c.bgRaised, borderRadius: 10, padding: 14 }}>
        {stats.code ? (
          <div style={{ display: 'flex', flexWrap: 'wrap', gap: 12, alignItems: 'center' }}>
            <span style={{ color: c.textSecondary }}>{t('referral.yourCode')}</span>
            <code
              dir="ltr"
              style={{ fontSize: '1.4rem', fontWeight: 700, letterSpacing: 2, color: c.accent }}
            >
              {formatReferralCode(stats.code)}
            </code>
            <button type="button" style={button(c.primaryFill)} onClick={() => void copy()}>
              {t('referral.copyLink')}
            </button>
            <span role="status" style={{ color: c.success }}>
              {copyStatus}
            </span>
          </div>
        ) : (
          <button
            type="button"
            style={button(c.primaryFill, stats.blocked || generating)}
            aria-busy={generating}
            aria-disabled={stats.blocked || undefined}
            onClick={() => !stats.blocked && !generating && onGenerateCode?.()}
          >
            {t('referral.generate')}
          </button>
        )}
      </div>

      {/* Stats */}
      <dl
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(120px, 1fr))',
          gap: 10,
          margin: 0,
        }}
      >
        {statItems.map(([label, value]) => (
          <div key={label} style={{ backgroundColor: c.bgRaised, borderRadius: 8, padding: 10 }}>
            <dt style={{ color: c.textMuted, fontSize: '0.85rem' }}>{label}</dt>
            <dd style={{ margin: 0, fontSize: '1.3rem', fontWeight: 700 }}>{value}</dd>
          </div>
        ))}
      </dl>

      {/* Tier */}
      <div>
        <p style={{ margin: '0 0 6px' }}>
          {t('referral.tier', {
            tier: t(`referral.tiers.${tier.name}`),
            multiplier: formatNumber(tier.multiplierBps / 10_000, { maximumFractionDigits: 1 }),
          })}
        </p>
        <div
          role="progressbar"
          aria-label={t('referral.tierProgress')}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={progress}
          aria-valuetext={
            next
              ? t('referral.toNextTier', {
                  count: next.minReferrals - stats.totalReferrals,
                  tier: t(`referral.tiers.${next.name}`),
                })
              : t('referral.maxTier')
          }
          style={{ height: 10, borderRadius: 5, backgroundColor: c.bgRaised, overflow: 'hidden' }}
        >
          <div style={{ width: `${progress}%`, height: '100%', backgroundColor: c.accent }} />
        </div>
        <p style={{ margin: '6px 0 0', color: c.textMuted, fontSize: '0.9rem' }}>
          {next
            ? t('referral.toNextTier', {
                count: next.minReferrals - stats.totalReferrals,
                tier: t(`referral.tiers.${next.name}`),
              })
            : t('referral.maxTier')}
        </p>
      </div>

      {/* Referees */}
      <div>
        <h3 style={{ margin: '0 0 8px', fontSize: '1rem' }}>{t('referral.referees')}</h3>
        {referees.length === 0 ? (
          <p style={{ margin: 0, color: c.textSecondary }}>{t('referral.noReferees')}</p>
        ) : (
          <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 8 }}>
            {referees.map((r) => {
              const status = refereeStatus(r);
              const busy = pendingClaim === r.address;
              return (
                <li
                  key={r.address}
                  style={{
                    display: 'flex',
                    flexWrap: 'wrap',
                    gap: 10,
                    alignItems: 'center',
                    justifyContent: 'space-between',
                    backgroundColor: c.bgRaised,
                    borderRadius: 8,
                    padding: '8px 12px',
                  }}
                >
                  <span>
                    <code dir="ltr" title={r.address}>
                      {shortAddress(r.address)}
                    </code>{' '}
                    <span style={{ color: c.textMuted, fontSize: '0.85rem' }}>
                      {t('referral.joined', { date: formatDate(r.registeredAt) })}
                    </span>
                  </span>
                  {status === 'claimable' ? (
                    <button
                      type="button"
                      aria-busy={busy}
                      aria-label={t('referral.claimFor', { address: shortAddress(r.address) })}
                      style={button(c.successFill, stats.blocked)}
                      aria-disabled={stats.blocked || undefined}
                      onClick={() => !busy && !stats.blocked && onClaim?.(r.address)}
                    >
                      {t('referral.claim')}
                    </button>
                  ) : (
                    <span
                      style={{
                        color: status === 'claimed' ? c.success : c.textSecondary,
                        fontWeight: 700,
                      }}
                    >
                      {t(`referral.status.${status}`)}
                    </span>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </div>

      {/* Redeem */}
      {onRedeemCode && (
        <form
          noValidate
          onSubmit={(e) => {
            e.preventDefault();
            if (!isValidReferralCode(codeInput)) {
              setCodeError(true);
              return;
            }
            setCodeError(false);
            onRedeemCode(normalizeReferralCode(codeInput));
          }}
          style={{ display: 'flex', flexWrap: 'wrap', gap: 8, alignItems: 'center' }}
        >
          <label htmlFor={codeInputId} style={{ fontWeight: 700 }}>
            {t('referral.haveCode')}
          </label>
          <input
            id={codeInputId}
            dir="ltr"
            value={codeInput}
            autoComplete="off"
            aria-invalid={codeError || undefined}
            aria-describedby={codeError ? codeErrorId : undefined}
            onChange={(e) => setCodeInput(e.target.value)}
            placeholder="ABCD-2345"
            style={{
              backgroundColor: c.bgRaised,
              color: c.textPrimary,
              border: `1px solid ${codeError ? c.danger : c.textMuted}`,
              borderRadius: 6,
              padding: '6px 8px',
              textTransform: 'uppercase',
            }}
          />
          <button type="submit" style={button(c.primaryFill)}>
            {t('referral.redeem')}
          </button>
          {codeError && (
            <span id={codeErrorId} role="alert" style={{ color: c.danger }}>
              {t('referral.invalidCode')}
            </span>
          )}
        </form>
      )}
    </section>
  );
};

/** @deprecated Use `ReferralPanel`. Kept for existing imports. */
export const Referrals = ReferralPanel;
