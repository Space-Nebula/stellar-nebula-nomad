/**
 * Admin review queue for bot detection (Issue #704): summary, filter, cases
 * sorted by risk with decoded signals, and clear / ban actions (ban needs a
 * second confirming click).
 */
import React, { useId, useState } from 'react';

import { useI18n } from '../i18n';
import { themeColors as c } from '../theme/themes';
import {
  BLOCK_THRESHOLD,
  CaseFilter,
  CaseVerdict,
  decodeSignals,
  responseForScore,
  ReviewCaseView,
  sortCases,
} from '../utils/botDetection';

export interface BotReviewDashboardProps {
  cases: ReviewCaseView[];
  now?: number;
  /** Case whose resolution is in flight. */
  pendingCaseId?: number;
  onResolve?: (caseId: number, verdict: CaseVerdict) => void;
  initialFilter?: CaseFilter;
}

const shortAddress = (a: string) => (a.length > 12 ? `${a.slice(0, 6)}…${a.slice(-4)}` : a);

const scoreColor = (score: number) =>
  score >= BLOCK_THRESHOLD ? c.danger : score >= 60 ? c.warning : score >= 30 ? c.accent : c.success;

const actionButton = (fill: string, busy: boolean): React.CSSProperties => ({
  backgroundColor: fill,
  color: c.textOnAccent,
  border: 'none',
  borderRadius: 6,
  padding: '6px 10px',
  fontWeight: 700,
  cursor: busy ? 'progress' : 'pointer',
  opacity: busy ? 0.7 : 1,
});

export const BotReviewDashboard: React.FC<BotReviewDashboardProps> = ({
  cases,
  now = Date.now(),
  pendingCaseId,
  onResolve,
  initialFilter = 'open',
}) => {
  const { t, formatNumber, formatRelativeTime } = useI18n();
  const titleId = useId();
  const filterId = useId();
  const [filter, setFilter] = useState<CaseFilter>(initialFilter);
  const [confirmBan, setConfirmBan] = useState<number | undefined>();

  const open = cases.filter((x) => x.status === 'Open');
  const highRisk = open.filter((x) => (x.currentScore ?? x.peakScore) >= BLOCK_THRESHOLD);
  const visible = sortCases(cases, filter);

  const summary: [string, number][] = [
    [t('bots.summary.open'), open.length],
    [t('bots.summary.highRisk'), highRisk.length],
    [t('bots.summary.cleared'), cases.filter((x) => x.status === 'Cleared').length],
    [t('bots.summary.banned'), cases.filter((x) => x.status === 'Banned').length],
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
      <div style={{ display: 'flex', justifyContent: 'space-between', flexWrap: 'wrap', gap: 8 }}>
        <h2 id={titleId} style={{ margin: 0, fontSize: '1.25rem' }}>
          {t('bots.title')}
        </h2>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <label htmlFor={filterId} style={{ color: c.textSecondary }}>
            {t('bots.filter')}
          </label>
          <select
            id={filterId}
            value={filter}
            onChange={(e) => setFilter(e.target.value as CaseFilter)}
            style={{
              backgroundColor: c.bgRaised,
              color: c.textPrimary,
              border: `1px solid ${c.textMuted}`,
              borderRadius: 6,
              padding: '4px 8px',
            }}
          >
            {(['open', 'resolved', 'all'] as CaseFilter[]).map((f) => (
              <option key={f} value={f}>
                {t(`bots.filters.${f}`)}
              </option>
            ))}
          </select>
        </div>
      </div>

      <dl
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(auto-fit, minmax(110px, 1fr))',
          gap: 10,
          margin: 0,
        }}
      >
        {summary.map(([label, value]) => (
          <div key={label} style={{ backgroundColor: c.bgRaised, borderRadius: 8, padding: 10 }}>
            <dt style={{ color: c.textMuted, fontSize: '0.85rem' }}>{label}</dt>
            <dd style={{ margin: 0, fontSize: '1.3rem', fontWeight: 700 }}>{formatNumber(value)}</dd>
          </div>
        ))}
      </dl>

      {visible.length === 0 ? (
        <p style={{ margin: 0, color: c.textSecondary }}>{t('bots.empty')}</p>
      ) : (
        <table style={{ width: '100%', borderCollapse: 'collapse' }}>
          <caption style={{ textAlign: 'start', color: c.textMuted, paddingBottom: 6 }}>
            {t('bots.caption')}
          </caption>
          <thead>
            <tr style={{ textAlign: 'start', color: c.textSecondary }}>
              <th scope="col" style={{ textAlign: 'start', padding: 6 }}>{t('bots.col.player')}</th>
              <th scope="col" style={{ textAlign: 'start', padding: 6 }}>{t('bots.col.score')}</th>
              <th scope="col" style={{ textAlign: 'start', padding: 6 }}>{t('bots.col.signals')}</th>
              <th scope="col" style={{ textAlign: 'start', padding: 6 }}>{t('bots.col.opened')}</th>
              <th scope="col" style={{ textAlign: 'start', padding: 6 }}>{t('bots.col.action')}</th>
            </tr>
          </thead>
          <tbody>
            {visible.map((x) => {
              const score = x.currentScore ?? x.peakScore;
              const busy = pendingCaseId === x.id;
              const response = responseForScore(score);
              return (
                <tr key={x.id} style={{ borderTop: `1px solid ${c.border}` }}>
                  <th scope="row" style={{ textAlign: 'start', padding: 6, fontWeight: 400 }}>
                    <code dir="ltr" title={x.player}>
                      {shortAddress(x.player)}
                    </code>
                    <div style={{ color: c.textMuted, fontSize: '0.8rem' }}>
                      {t('bots.case', { id: x.id })}
                    </div>
                  </th>
                  <td style={{ padding: 6 }}>
                    <span style={{ color: scoreColor(score), fontWeight: 700 }}>
                      {formatNumber(score)}
                    </span>
                    <span style={{ color: c.textMuted }}> / 100</span>
                    <div style={{ fontSize: '0.8rem', color: c.textSecondary }}>
                      {t(`bots.response.${response}`)}
                    </div>
                  </td>
                  <td style={{ padding: 6 }}>
                    <ul
                      aria-label={t('bots.col.signals')}
                      style={{ listStyle: 'none', padding: 0, margin: 0, display: 'flex', flexWrap: 'wrap', gap: 4 }}
                    >
                      {decodeSignals(x.signals).map((s) => (
                        <li
                          key={s}
                          style={{
                            backgroundColor: c.bgRaised,
                            border: `1px solid ${c.border}`,
                            borderRadius: 999,
                            padding: '2px 8px',
                            fontSize: '0.8rem',
                          }}
                        >
                          {t(`bots.signal.${s}`)}
                        </li>
                      ))}
                    </ul>
                  </td>
                  <td style={{ padding: 6, color: c.textSecondary }}>
                    {formatRelativeTime(x.openedAt, now)}
                  </td>
                  <td style={{ padding: 6 }}>
                    {x.status !== 'Open' ? (
                      <span style={{ fontWeight: 700, color: x.status === 'Banned' ? c.danger : c.success }}>
                        {t(`bots.status.${x.status}`)}
                      </span>
                    ) : (
                      <div style={{ display: 'flex', gap: 6, flexWrap: 'wrap' }}>
                        <button
                          type="button"
                          aria-busy={busy}
                          aria-label={t('bots.clearCase', { id: x.id })}
                          style={actionButton(c.successFill, busy)}
                          onClick={() => {
                            if (busy) return;
                            setConfirmBan(undefined);
                            onResolve?.(x.id, 'Clear');
                          }}
                        >
                          {t('bots.clear')}
                        </button>
                        {confirmBan === x.id ? (
                          <>
                            <button
                              type="button"
                              aria-busy={busy}
                              aria-label={t('bots.confirmBanCase', { id: x.id })}
                              style={actionButton(c.dangerFill, busy)}
                              onClick={() => {
                                if (busy) return;
                                setConfirmBan(undefined);
                                onResolve?.(x.id, 'Ban');
                              }}
                            >
                              {t('bots.confirmBan')}
                            </button>
                            <button
                              type="button"
                              style={{ ...actionButton(c.border, false), color: c.textPrimary }}
                              onClick={() => setConfirmBan(undefined)}
                            >
                              {t('bots.cancel')}
                            </button>
                          </>
                        ) : (
                          <button
                            type="button"
                            aria-label={t('bots.banCase', { id: x.id })}
                            style={actionButton(c.dangerFill, busy)}
                            onClick={() => !busy && setConfirmBan(x.id)}
                          >
                            {t('bots.ban')}
                          </button>
                        )}
                      </div>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      )}
    </section>
  );
};
