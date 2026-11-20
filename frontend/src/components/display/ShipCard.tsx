import React from 'react';
import { colors } from '../../a11y';
import { useI18n } from '../../i18n';

export interface ShipCardProps {
  id: string;
  name: string;
  shipClass: 'Scout' | 'Frigate' | 'Cruiser' | 'Dreadnought';
  rarity: 'Common' | 'Rare' | 'Epic' | 'Legendary';
  level: number;
  stats: {
    speed: number;
    hull: number;
    scanPower: number;
  };
  onSelect?: (id: string) => void;
}

const rarityColors: Record<ShipCardProps['rarity'], string> = {
  Common: colors.textMuted,
  Rare: colors.accent,
  Epic: colors.epic,
  Legendary: colors.warning,
};

export const ShipCard: React.FC<ShipCardProps> = ({
  id,
  name,
  shipClass,
  rarity,
  level,
  stats,
  onSelect,
}) => {
  const { t, formatNumber } = useI18n();
  const color = rarityColors[rarity] || colors.textMuted;
  const rarityText = t(`rarity.${rarity}`);
  const classText = t(`ship.class.${shipClass}`);

  const stat = (label: string, value: number, valueColor: string) => (
    <div
      style={{
        backgroundColor: colors.bgSurface,
        padding: '8px',
        borderRadius: '6px',
        textAlign: 'center',
      }}
    >
      <div style={{ color: colors.textMuted }}>{label}</div>
      <div style={{ fontWeight: 700, color: valueColor }}>{formatNumber(value)}</div>
    </div>
  );

  return (
    <div
      role="article"
      aria-label={t('ship.label', { name, rarity: rarityText, shipClass: classText })}
      onClick={() => onSelect?.(id)}
      style={{
        backgroundColor: colors.bgRaised,
        border: `2px solid ${color}`,
        borderRadius: '12px',
        padding: '16px',
        color: colors.textPrimary,
        display: 'flex',
        flexDirection: 'column',
        gap: '12px',
        cursor: onSelect ? 'pointer' : 'default',
        transition: 'transform 0.2s ease, box-shadow 0.2s ease',
        position: 'relative',
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <h3 style={{ margin: 0, fontSize: '1.1rem', color: colors.textPrimary }}>{name}</h3>
        <span
          style={{
            backgroundColor: `${color}22`,
            color,
            padding: '2px 8px',
            borderRadius: '9999px',
            fontSize: '0.75rem',
            fontWeight: 700,
          }}
        >
          {rarityText}
        </span>
      </div>

      <div style={{ fontSize: '0.85rem', color: colors.textMuted }}>
        {classText} • {t('ship.level', { level: formatNumber(level) })}
      </div>

      <div
        style={{
          display: 'grid',
          gridTemplateColumns: 'repeat(3, 1fr)',
          gap: '8px',
          fontSize: '0.75rem',
        }}
      >
        {stat(t('ship.speed'), stats.speed, colors.accent)}
        {stat(t('ship.hull'), stats.hull, colors.success)}
        {stat(t('ship.scan'), stats.scanPower, colors.epic)}
      </div>

      {onSelect && (
        // Keyboard and screen-reader users get a real button; mouse users can
        // click anywhere on the card. `AccessibilityStyles` hides the button
        // visually until it receives focus.
        <button
          type="button"
          className="snn-reveal-on-focus"
          onClick={(event) => {
            event.stopPropagation();
            onSelect(id);
          }}
          style={selectButtonStyle}
        >
          {t('ship.select', { name })}
        </button>
      )}
    </div>
  );
};

const selectButtonStyle: React.CSSProperties = {
  position: 'absolute',
  insetInlineEnd: 8,
  bottom: 8,
  padding: '4px 10px',
  whiteSpace: 'nowrap',
  border: 'none',
  borderRadius: 6,
  backgroundColor: colors.primaryFill,
  color: colors.textOnAccent,
  fontWeight: 700,
};
