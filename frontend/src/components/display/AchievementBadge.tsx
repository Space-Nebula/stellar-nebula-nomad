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
  rarityTier?: 0 | 1 | 2 | 3 | 4;
  category?: string;
  progressPct?: number;
}

const RARITY_BORDER: Record<number, string> = {
  0: '#64748B',
  1: '#10B981',
  2: '#3B82F6',
  3: '#A855F7',
  4: '#F59E0B',
};

export const AchievementBadge: React.FC<AchievementBadgeProps> = ({
  title,
  description,
  unlocked,
  unlockedAt,
  icon = '🏆',
  rarityTier = 2,
  category,
  progressPct,
}) => {
  const borderColor = unlocked ? (RARITY_BORDER[rarityTier] ?? '#3B82F6') : '#334155';

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
        backgroundColor: unlocked ? '#1E293B' : '#0F172A',
        border: `1px solid ${borderColor}`,
        opacity: unlocked ? 1 : 0.6,
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
        <span style={{ fontWeight: 700, color: unlocked ? '#F8FAFC' : '#94A3B8', fontSize: '0.9rem' }}>
          {title} {category ? `[${category}]` : ''}
        </span>
        <span style={{ color: '#64748B', fontSize: '0.75rem' }}>{description}</span>
        {!unlocked && typeof progressPct === 'number' && (
          <span style={{ color: '#94A3B8', fontSize: '0.7rem', marginTop: '2px' }}>
            Progress: {progressPct}%
          </span>
        )}
        {unlocked && unlockedAt && (
          <span style={{ color: '#10B981', fontSize: '0.7rem', marginTop: '2px' }}>
            Unlocked on {unlockedAt}
          </span>
        )}
      </div>
    </div>
  );
};
