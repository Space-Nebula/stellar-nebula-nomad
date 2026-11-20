import React from 'react';
import { colors } from '../../a11y';
import { useI18n } from '../../i18n';

export interface AchievementBadgeProps {
  id: string;
  title: string;
  description: string;
  unlocked: boolean;
  unlockedAt?: string;
  icon?: string;
}

export const AchievementBadge: React.FC<AchievementBadgeProps> = ({
  title,
  description,
  unlocked,
  unlockedAt,
  icon = '🏆',
}) => {
  const { t } = useI18n();
  const status = t(unlocked ? 'achievement.unlocked' : 'achievement.locked');
  return (
    // A static badge is not a live region: use a labelled group, not role="status".
    <div
      role="group"
      aria-label={t('achievement.label', { title, status })}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: '12px',
        padding: '12px',
        borderRadius: '8px',
        backgroundColor: unlocked ? colors.bgRaised : colors.bgSurface,
        border: `1px ${unlocked ? 'solid' : 'dashed'} ${unlocked ? colors.accent : colors.textMuted}`,
      }}
    >
      <div
        aria-hidden="true"
        style={{
          fontSize: '1.75rem',
          filter: unlocked ? 'none' : 'grayscale(100%)',
          opacity: unlocked ? 1 : 0.6,
        }}
      >
        {icon}
      </div>
      <div style={{ display: 'flex', flexDirection: 'column' }}>
        <span
          style={{
            fontWeight: 700,
            color: unlocked ? colors.textPrimary : colors.textMuted,
            fontSize: '0.9rem',
          }}
        >
          {title}
        </span>
        <span style={{ color: colors.textMuted, fontSize: '0.75rem' }}>{description}</span>
        <span style={{ color: unlocked ? colors.success : colors.textMuted, fontSize: '0.7rem', marginTop: '2px' }}>
          {unlocked && unlockedAt ? t('achievement.unlockedOn', { date: unlockedAt }) : status}
        </span>
      </div>
    </div>
  );
};
