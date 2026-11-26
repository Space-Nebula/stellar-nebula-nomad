import React from 'react';
import { colors } from '../../a11y';
import { useI18n } from '../../i18n';

export interface NebulaVisualizationProps {
  sectorName: string;
  density: number;
  anomalyDetected: boolean;
  colorTheme?: 'cyan' | 'magenta' | 'amber';
}

export const NebulaVisualization: React.FC<NebulaVisualizationProps> = ({
  sectorName,
  density,
  anomalyDetected,
  colorTheme = 'cyan',
}) => {
  const { t, formatPercent } = useI18n();
  const themeGradients = {
    cyan: 'radial-gradient(circle at 50% 50%, rgba(56, 189, 248, 0.4), rgba(15, 23, 42, 0.95))',
    magenta: 'radial-gradient(circle at 50% 50%, rgba(217, 70, 239, 0.4), rgba(15, 23, 42, 0.95))',
    amber: 'radial-gradient(circle at 50% 50%, rgba(245, 158, 11, 0.4), rgba(15, 23, 42, 0.95))',
  };
  const densityText = formatPercent(density / 100);
  const label =
    t('nebula.label', { sector: sectorName, density: densityText }) +
    (anomalyDetected ? `, ${t('nebula.anomalyLabel')}` : '');

  return (
    // role="img" hides the children from assistive tech, so the label carries
    // everything the visual shows (sector, density, anomaly).
    <div
      role="img"
      aria-label={label}
      style={{
        width: '100%',
        height: '180px',
        borderRadius: '12px',
        background: themeGradients[colorTheme],
        border: `1px solid ${colors.border}`,
        position: 'relative',
        overflow: 'hidden',
        display: 'flex',
        flexDirection: 'column',
        justifyContent: 'space-between',
        padding: '16px',
        boxSizing: 'border-box',
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span style={{ fontWeight: 700, color: colors.textPrimary }}>{sectorName}</span>
        {anomalyDetected && (
          <span
            style={{
              backgroundColor: colors.dangerFill,
              color: colors.textOnAccent,
              fontSize: '0.7rem',
              padding: '2px 6px',
              borderRadius: '4px',
              fontWeight: 700,
            }}
          >
            {t('nebula.anomaly')}
          </span>
        )}
      </div>

      <div style={{ color: colors.textSecondary, fontSize: '0.8rem' }}>
        {t('nebula.density', { density: densityText })}
      </div>
    </div>
  );
};
