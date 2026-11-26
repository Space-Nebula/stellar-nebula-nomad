import React, { useId } from 'react';
import { colors } from '../../a11y';
import { useI18n } from '../../i18n';

export type RarityType = 'All' | 'Common' | 'Rare' | 'Epic' | 'Legendary';

export interface RaritySelectorProps {
  selected: RarityType;
  onChange: (rarity: RarityType) => void;
  includeAll?: boolean;
}

export const RaritySelector: React.FC<RaritySelectorProps> = ({
  selected,
  onChange,
  includeAll = true,
}) => {
  const { t } = useI18n();
  const labelId = useId();
  const rarities: RarityType[] = includeAll
    ? ['All', 'Common', 'Rare', 'Epic', 'Legendary']
    : ['Common', 'Rare', 'Epic', 'Legendary'];

  return (
    <div
      role="group"
      aria-labelledby={labelId}
      style={{ display: 'flex', flexDirection: 'column', gap: '6px' }}
    >
      <span id={labelId} style={{ fontSize: '0.8rem', color: colors.textMuted }}>
        {t('rarity.label')}
      </span>
      <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px' }}>
        {rarities.map((r) => {
          const isSelected = selected === r;
          return (
            <button
              key={r}
              type="button"
              aria-pressed={isSelected}
              onClick={() => onChange(r)}
              style={{
                backgroundColor: isSelected ? colors.selectedFill : colors.bgSurface,
                color: isSelected ? colors.textOnAccent : colors.textMuted,
                border: `1px solid ${isSelected ? colors.selectedFill : colors.textMuted}`,
                borderRadius: '6px',
                padding: '4px 10px',
                fontSize: '0.75rem',
                cursor: 'pointer',
                fontWeight: 600,
              }}
            >
              {t(`rarity.${r}`)}
            </button>
          );
        })}
      </div>
    </div>
  );
};
