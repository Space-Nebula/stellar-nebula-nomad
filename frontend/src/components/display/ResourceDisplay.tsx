import React, { useId } from 'react';
import { colors } from '../../a11y';
import { useI18n } from '../../i18n';

export interface ResourceDisplayProps {
  name: string;
  amount: number;
  icon?: string;
  change24h?: number;
  maxCapacity?: number;
}

export const ResourceDisplay: React.FC<ResourceDisplayProps> = ({
  name,
  amount,
  icon = '💎',
  change24h,
  maxCapacity,
}) => {
  const { t, formatNumber } = useI18n();
  const nameId = useId();
  const percentage = maxCapacity ? Math.min(Math.round((amount / maxCapacity) * 100), 100) : null;

  return (
    <div
      role="group"
      aria-labelledby={nameId}
      style={{
        backgroundColor: colors.bgSurface,
        border: `1px solid ${colors.border}`,
        borderRadius: '8px',
        padding: '12px 16px',
        display: 'flex',
        flexDirection: 'column',
        gap: '6px',
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
          <span aria-hidden="true" style={{ fontSize: '1.2rem' }}>
            {icon}
          </span>
          <span id={nameId} style={{ color: colors.textMuted, fontSize: '0.85rem', fontWeight: 600 }}>
            {name}
          </span>
        </div>
        {change24h !== undefined && (
          // The sign conveys direction, so colour is not the only cue.
          <span
            dir="ltr"
            style={{
              fontSize: '0.75rem',
              fontWeight: 600,
              color: change24h >= 0 ? colors.success : colors.danger,
            }}
          >
            {change24h >= 0 ? `+${change24h}%` : `${change24h}%`}
          </span>
        )}
      </div>

      <div style={{ display: 'flex', alignItems: 'baseline', gap: '6px' }}>
        <span style={{ fontSize: '1.25rem', fontWeight: 700, color: colors.textPrimary }}>
          {formatNumber(amount)}
        </span>
        {maxCapacity && (
          <span style={{ fontSize: '0.75rem', color: colors.textMuted }}>
            / {formatNumber(maxCapacity)}
          </span>
        )}
      </div>

      {percentage !== null && (
        <div
          role="progressbar"
          aria-label={t('resource.capacity', { name })}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={percentage}
          style={{
            backgroundColor: colors.bgRaised,
            height: '6px',
            borderRadius: '3px',
            overflow: 'hidden',
          }}
        >
          <div
            style={{
              width: `${percentage}%`,
              height: '100%',
              backgroundColor: percentage > 90 ? colors.danger : colors.accent,
            }}
          />
        </div>
      )}
    </div>
  );
};
