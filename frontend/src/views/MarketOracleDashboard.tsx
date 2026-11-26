import React, { useId } from 'react';
import { colors } from '../a11y';
import { useI18n } from '../i18n';

/**
 * `fresh`: consensus price within the 24h freshness window.
 * `stale`: last price is older than 24h; contracts refuse to use it.
 * `rejected`: the latest aggregation failed (disagreement or circuit
 * breaker); the previous validated price is still shown.
 */
export type OraclePriceStatus = 'fresh' | 'stale' | 'rejected';

export interface OraclePrice {
  resource: string;
  /** Published consensus price. */
  price: number;
  /** Agreeing sources behind the price (`PriceData.source_count`). */
  sourceCount: number;
  /** Publication time in milliseconds since the epoch. */
  updatedAt: number;
  status: OraclePriceStatus;
}

export interface MarketOracleDashboardProps {
  prices: OraclePrice[];
  /** Reference time for relative timestamps (ms); defaults to now. */
  now?: number;
}

const statusStyle: Record<OraclePriceStatus, { color: string; icon: string }> = {
  fresh: { color: colors.success, icon: '✓' },
  stale: { color: colors.warning, icon: '!' },
  rejected: { color: colors.danger, icon: '✕' },
};

export const MarketOracleDashboard: React.FC<MarketOracleDashboardProps> = ({ prices, now }) => {
  const { t, formatNumber, formatDate, formatRelativeTime } = useI18n();
  const titleId = useId();
  const descriptionId = useId();
  const cell: React.CSSProperties = {
    padding: '10px 12px',
    borderBottom: `1px solid ${colors.bgRaised}`,
    textAlign: 'start',
  };

  return (
    <section
      aria-labelledby={titleId}
      aria-describedby={descriptionId}
      style={{
        backgroundColor: colors.bgSurface,
        border: `1px solid ${colors.border}`,
        borderRadius: 12,
        padding: 20,
        color: colors.textPrimary,
      }}
    >
      <h2 id={titleId} style={{ margin: 0, fontSize: '1.25rem' }}>
        {t('oracle.title')}
      </h2>
      <p id={descriptionId} style={{ color: colors.textMuted, marginTop: 4 }}>
        {t('oracle.description')}
      </p>

      {prices.length === 0 ? (
        <p style={{ color: colors.textSecondary }}>{t('oracle.empty')}</p>
      ) : (
        <div style={{ overflowX: 'auto' }}>
          <table style={{ width: '100%', borderCollapse: 'collapse', fontSize: '0.9rem' }}>
            <caption style={{ textAlign: 'start', color: colors.textMuted, paddingBottom: 8 }}>
              {t('oracle.pricesLabel')}
            </caption>
            <thead>
              <tr style={{ color: colors.textMuted }}>
                <th scope="col" style={cell}>
                  {t('oracle.resource')}
                </th>
                <th scope="col" style={{ ...cell, textAlign: 'end' }}>
                  {t('oracle.consensus')}
                </th>
                <th scope="col" style={cell}>
                  {t('oracle.sourcesHeader')}
                </th>
                <th scope="col" style={cell}>
                  {t('oracle.updatedHeader')}
                </th>
                <th scope="col" style={cell}>
                  {t('oracle.statusHeader')}
                </th>
              </tr>
            </thead>
            <tbody>
              {prices.map((price) => {
                const style = statusStyle[price.status];
                return (
                  <tr key={price.resource}>
                    <th scope="row" style={{ ...cell, fontWeight: 700 }}>
                      {price.resource}
                    </th>
                    <td style={{ ...cell, textAlign: 'end', fontVariantNumeric: 'tabular-nums' }}>
                      {formatNumber(price.price)}
                    </td>
                    <td style={cell}>{t('oracle.sources', { count: price.sourceCount })}</td>
                    <td style={cell}>
                      <time
                        dateTime={new Date(price.updatedAt).toISOString()}
                        title={formatDate(price.updatedAt)}
                      >
                        {formatRelativeTime(price.updatedAt, now)}
                      </time>
                    </td>
                    <td style={cell}>
                      <span
                        style={{
                          display: 'inline-flex',
                          alignItems: 'center',
                          gap: 6,
                          color: style.color,
                          fontWeight: 700,
                        }}
                      >
                        <span aria-hidden="true">{style.icon}</span>
                        {t(`oracle.status.${price.status}`)}
                      </span>
                      {price.status === 'rejected' && (
                        <div style={{ color: colors.textMuted, fontSize: '0.8rem' }}>
                          {t('oracle.rejectedReason')}
                        </div>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </section>
  );
};
