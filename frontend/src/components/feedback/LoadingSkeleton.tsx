import React from 'react';
import { colors, visuallyHidden } from '../../a11y';
import { useI18n } from '../../i18n';

export interface LoadingSkeletonProps {
  width?: string | number;
  height?: string | number;
  borderRadius?: string | number;
  count?: number;
  /** Text announced to screen readers (defaults to "Loading..."). */
  label?: string;
}

export const LoadingSkeleton: React.FC<LoadingSkeletonProps> = ({
  width = '100%',
  height = '20px',
  borderRadius = '6px',
  count = 1,
  label,
}) => {
  const { t } = useI18n();
  const items = Array.from({ length: count });

  return (
    <div
      role="status"
      aria-live="polite"
      aria-busy="true"
      style={{ display: 'flex', flexDirection: 'column', gap: '8px', width: '100%' }}
    >
      <span style={visuallyHidden}>{label ?? t('common.loading')}</span>
      {items.map((_, i) => (
        <div
          key={i}
          aria-hidden="true"
          style={{
            width,
            height,
            borderRadius,
            backgroundColor: colors.bgRaised,
            opacity: 0.7,
            animation: 'pulse 1.5s cubic-bezier(0.4, 0, 0.6, 1) infinite',
          }}
        />
      ))}
    </div>
  );
};
