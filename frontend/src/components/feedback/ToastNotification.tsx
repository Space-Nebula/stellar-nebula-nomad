import React from 'react';
import { colors } from '../../a11y';
import { useI18n } from '../../i18n';

export interface ToastProps {
  id: string;
  title: string;
  message: string;
  type?: 'info' | 'success' | 'warning' | 'error';
  onDismiss: (id: string) => void;
}

const typeColors = {
  info: colors.accent,
  success: colors.success,
  warning: colors.warning,
  error: colors.danger,
};

export const ToastNotification: React.FC<ToastProps> = ({
  id,
  title,
  message,
  type = 'info',
  onDismiss,
}) => {
  const { t } = useI18n();
  const color = typeColors[type];
  // Only interrupt screen-reader users for problems; other toasts are polite.
  const urgent = type === 'error' || type === 'warning';

  return (
    <div
      role={urgent ? 'alert' : 'status'}
      aria-live={urgent ? 'assertive' : 'polite'}
      aria-atomic="true"
      style={{
        display: 'flex',
        alignItems: 'flex-start',
        justifyContent: 'space-between',
        padding: '12px 16px',
        backgroundColor: colors.bgRaised,
        borderInlineStart: `4px solid ${color}`,
        borderRadius: '8px',
        boxShadow: '0 10px 15px -3px rgba(0, 0, 0, 0.4)',
        color: colors.textPrimary,
        minWidth: '280px',
        maxWidth: '400px',
      }}
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: '2px' }}>
        <span style={{ fontWeight: 700, fontSize: '0.875rem' }}>{title}</span>
        <span style={{ color: colors.textMuted, fontSize: '0.8rem' }}>{message}</span>
      </div>
      <button
        type="button"
        onClick={() => onDismiss(id)}
        aria-label={t('toast.dismiss')}
        style={{
          background: 'none',
          border: 'none',
          color: colors.textMuted,
          cursor: 'pointer',
          fontSize: '1rem',
          padding: '2px',
          minWidth: 24,
          minHeight: 24,
        }}
      >
        <span aria-hidden="true">✕</span>
      </button>
    </div>
  );
};
