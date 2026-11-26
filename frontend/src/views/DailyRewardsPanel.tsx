/**
 * Daily login rewards (Issue #703): streak, event banner, claim card and the
 * 28-day reward calendar.
 */
import React, { useId } from 'react';

import { useI18n } from '../i18n';
import { themeColors as c } from '../theme/themes';
import {
  buildCalendar,
  RewardEventView,
  RewardKind,
  resolveReward,
  splitDuration,
} from '../utils/dailyRewards';

/** Mirrors `daily_rewards::LoginStatus` (timestamps in ms). */
export interface LoginStatusView {
  canClaim: boolean;
  currentStreak: number;
  streakIfClaimed: number;
  streakAtRisk: boolean;
  /** When the next claim opens, ms. */
  nextClaimAt: number;
  /** When today's window closes, ms. */
  resetsAt: number;
  nextCalendarDay: number;
  longestStreak?: number;
}

export interface DailyRewardsPanelProps {
  status: LoginStatusView;
  /** Event active today, if any. */
  event?: RewardEventView;
  now?: number;
  claiming?: boolean;
  onClaim?: () => void;
}

const kindIcon: Record<RewardKind, string> = {
  Essence: '✦',
  StellarDust: '✧',
  DarkMatter: '●',
  ExoticMatter: '◆',
  Energy: '⚡',
  CosmeticRoll: '🎁',
};

export const DailyRewardsPanel: React.FC<DailyRewardsPanelProps> = ({
  status,
  event,
  now = Date.now(),
  claiming,
  onClaim,
}) => {
  const { t, formatNumber, formatDate } = useI18n();
  const titleId = useId();
  const calendarId = useId();

  const activeEvent = event && event.startsAt <= now && now < event.endsAt ? event : undefined;
  const next = resolveReward(status.nextCalendarDay, status.streakIfClaimed, activeEvent);
  const calendar = buildCalendar(status.nextCalendarDay, status.streakIfClaimed, activeEvent);

  const wait = splitDuration(status.nextClaimAt - now);
  const left = splitDuration(status.resetsAt - now);

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
      <div style={{ display: 'flex', justifyContent: 'space-between', flexWrap: 'wrap', gap: 8 }}>
        <h2 id={titleId} style={{ margin: 0, fontSize: '1.25rem' }}>
          {t('daily.title')}
        </h2>
        <p style={{ margin: 0, fontWeight: 700, color: c.warning }}>
          <span aria-hidden="true">🔥 </span>
          {t('daily.streak', { count: status.currentStreak })}
          {status.longestStreak !== undefined && (
            <span style={{ color: c.textMuted, fontWeight: 400 }}>
              {' · '}
              {t('daily.best', { count: status.longestStreak })}
            </span>
          )}
        </p>
      </div>

      {activeEvent && (
        <div
          role="note"
          style={{
            backgroundColor: c.secondaryFill,
            color: c.textOnAccent,
            borderRadius: 10,
            padding: '10px 14px',
          }}
        >
          <strong>{t('daily.eventTitle', { name: activeEvent.name })}</strong>{' '}
          {t('daily.eventDetail', {
            multiplier: formatNumber(activeEvent.multiplierBps / 10_000, { maximumFractionDigits: 2 }),
            bonus: formatNumber(activeEvent.bonus),
            date: formatDate(activeEvent.endsAt - 1),
          })}
        </div>
      )}

      <div
        style={{
          backgroundColor: c.bgRaised,
          borderRadius: 10,
          padding: 14,
          display: 'flex',
          flexWrap: 'wrap',
          gap: 12,
          alignItems: 'center',
          justifyContent: 'space-between',
        }}
      >
        <div>
          <p style={{ margin: 0, color: c.textSecondary }}>
            {status.canClaim ? t('daily.todayReward') : t('daily.nextReward')}
          </p>
          <p style={{ margin: 0, fontSize: '1.4rem', fontWeight: 700 }}>
            <span aria-hidden="true">{kindIcon[next.kind]} </span>
            {t('daily.amount', { amount: formatNumber(next.amount), kind: t(`daily.kind.${next.kind}`) })}
          </p>
          {next.streakBonusBps > 0 && (
            <p style={{ margin: 0, color: c.success, fontSize: '0.9rem' }}>
              {t('daily.streakBonus', { percent: formatNumber(next.streakBonusBps / 100) })}
            </p>
          )}
        </div>
        {status.canClaim ? (
          <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'flex-end', gap: 4 }}>
            <button
              type="button"
              aria-busy={claiming}
              onClick={() => !claiming && onClaim?.()}
              style={{
                backgroundColor: c.successFill,
                color: c.textOnAccent,
                border: 'none',
                borderRadius: 8,
                padding: '10px 18px',
                fontWeight: 700,
                fontSize: '1rem',
                cursor: claiming ? 'progress' : 'pointer',
                opacity: claiming ? 0.7 : 1,
              }}
            >
              {t('daily.claim')}
            </button>
            {status.streakAtRisk && (
              <span role="alert" style={{ color: c.warning, fontSize: '0.9rem' }}>
                {t('daily.atRisk', { hours: left.hours, minutes: left.minutes })}
              </span>
            )}
          </div>
        ) : (
          <p role="status" style={{ margin: 0, color: c.textSecondary }}>
            <span aria-hidden="true">✓ </span>
            {t('daily.claimedToday', { hours: wait.hours, minutes: wait.minutes })}
          </p>
        )}
      </div>

      <div>
        <h3 id={calendarId} style={{ margin: '0 0 8px', fontSize: '1rem' }}>
          {t('daily.calendar')}
        </h3>
        <ol
          aria-labelledby={calendarId}
          style={{
            listStyle: 'none',
            padding: 0,
            margin: 0,
            display: 'grid',
            gridTemplateColumns: 'repeat(7, minmax(0, 1fr))',
            gap: 6,
          }}
        >
          {calendar.map((slot) => {
            const stateLabel = t(`daily.state.${slot.state}`);
            const label = [
              t('daily.day', { day: slot.calendarDay }),
              t('daily.amount', { amount: formatNumber(slot.amount), kind: t(`daily.kind.${slot.kind}`) }),
              slot.isMilestone ? t('daily.milestone') : null,
              stateLabel,
            ]
              .filter(Boolean)
              .join(', ');
            const highlight = slot.state === 'next';
            return (
              <li
                key={slot.calendarDay}
                aria-label={label}
                aria-current={highlight ? 'date' : undefined}
                style={{
                  backgroundColor: slot.state === 'claimed' ? c.bgApp : c.bgRaised,
                  border: `2px solid ${highlight ? c.focus : slot.isMilestone ? c.epic : c.border}`,
                  borderRadius: 8,
                  padding: 6,
                  textAlign: 'center',
                  color: slot.state === 'claimed' ? c.textMuted : c.textPrimary,
                  fontSize: '0.8rem',
                }}
              >
                <div aria-hidden="true" style={{ fontWeight: 700 }}>
                  {slot.calendarDay}
                </div>
                <div aria-hidden="true" style={{ fontSize: '1.1rem' }}>
                  {slot.state === 'claimed' ? '✓' : kindIcon[slot.kind]}
                </div>
                <div aria-hidden="true">{formatNumber(slot.amount)}</div>
              </li>
            );
          })}
        </ol>
      </div>
    </section>
  );
};
