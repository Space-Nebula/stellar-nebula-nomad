import React, { useId } from 'react';
import { colors } from '../../a11y';
import { useI18n } from '../../i18n';

export interface DurationPickerProps {
  valueHours: number;
  onChange: (hours: number) => void;
  options?: number[];
}

export const DurationPicker: React.FC<DurationPickerProps> = ({
  valueHours,
  onChange,
  options = [1, 4, 8, 12, 24, 72],
}) => {
  const { t } = useI18n();
  const labelId = useId();
  return (
    <div
      role="group"
      aria-labelledby={labelId}
      style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}
    >
      <span id={labelId} style={{ fontSize: '0.8rem', color: colors.textMuted }}>
        {t('duration.label')}
      </span>
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '8px' }}>
        {options.map((h) => {
          const isSelected = valueHours === h;
          const isDays = h >= 24 && h % 24 === 0;
          const count = isDays ? h / 24 : h;
          return (
            <button
              key={h}
              type="button"
              aria-pressed={isSelected}
              onClick={() => onChange(h)}
              style={{
                backgroundColor: isSelected ? colors.primaryFill : colors.bgRaised,
                color: isSelected ? colors.textOnAccent : colors.textMuted,
                border: `1px solid ${isSelected ? colors.primaryFill : colors.textMuted}`,
                borderRadius: '6px',
                padding: '6px 12px',
                fontSize: '0.8rem',
                cursor: 'pointer',
                fontWeight: 600,
              }}
            >
              {t(isDays ? 'duration.days' : 'duration.hours', { count })}
            </button>
          );
        })}
      </div>
    </div>
  );
};
