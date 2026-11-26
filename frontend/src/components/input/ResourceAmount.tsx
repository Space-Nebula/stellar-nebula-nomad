import React, { useId } from 'react';
import { colors } from '../../a11y';
import { useI18n } from '../../i18n';

export interface ResourceAmountProps {
  value: number;
  onChange: (val: number) => void;
  min?: number;
  max?: number;
  step?: number;
  resourceName?: string;
  error?: string;
}

export const ResourceAmount: React.FC<ResourceAmountProps> = ({
  value,
  onChange,
  min = 0,
  max = 1000000,
  step = 1,
  resourceName = 'XLM',
  error,
}) => {
  const { t } = useI18n();
  const inputId = useId();
  const errorId = `${inputId}-error`;

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const parsed = Number(e.target.value);
    if (!isNaN(parsed)) {
      onChange(Math.max(min, Math.min(max, parsed)));
    }
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '4px', width: '100%' }}>
      <label htmlFor={inputId} style={{ fontSize: '0.8rem', color: colors.textMuted }}>
        {t('amount.label', { resource: resourceName })}
      </label>
      <div style={{ display: 'flex', alignItems: 'center', gap: '6px' }}>
        <input
          id={inputId}
          type="number"
          value={value}
          onChange={handleChange}
          min={min}
          max={max}
          step={step}
          aria-invalid={!!error}
          aria-describedby={error ? errorId : undefined}
          style={{
            flex: 1,
            backgroundColor: colors.bgSurface,
            border: `1px solid ${error ? colors.danger : colors.textMuted}`,
            borderRadius: '6px',
            padding: '8px 12px',
            color: colors.textPrimary,
            fontSize: '0.9rem',
          }}
        />
        <button
          type="button"
          onClick={() => onChange(max)}
          style={{
            backgroundColor: colors.bgRaised,
            border: `1px solid ${colors.textMuted}`,
            color: colors.accent,
            padding: '8px 12px',
            borderRadius: '6px',
            fontSize: '0.75rem',
            fontWeight: 700,
            cursor: 'pointer',
          }}
        >
          {t('amount.max')}
        </button>
      </div>
      {error && (
        <span id={errorId} role="alert" style={{ color: colors.danger, fontSize: '0.75rem' }}>
          {error}
        </span>
      )}
    </div>
  );
};
