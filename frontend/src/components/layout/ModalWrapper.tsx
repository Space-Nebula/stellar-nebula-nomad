import React, { useEffect, useId, useRef } from 'react';
import { colors, getFocusable, trapTabKey } from '../../a11y';
import { useI18n } from '../../i18n';

export interface ModalWrapperProps {
  isOpen: boolean;
  onClose: () => void;
  title?: string;
  /** Accessible name when there is no visible title. */
  ariaLabel?: string;
  children: React.ReactNode;
}

/**
 * Modal dialog (WAI-ARIA dialog pattern): moves focus into the dialog on
 * open, traps Tab/Shift+Tab, closes on Escape or backdrop click, and returns
 * focus to the element that opened it.
 */
export const ModalWrapper: React.FC<ModalWrapperProps> = ({
  isOpen,
  onClose,
  title,
  ariaLabel,
  children,
}) => {
  const { t } = useI18n();
  const titleId = useId();
  const panelRef = useRef<HTMLDivElement>(null);
  const onCloseRef = useRef(onClose);
  onCloseRef.current = onClose;

  useEffect(() => {
    if (!isOpen) return undefined;
    const previouslyFocused = document.activeElement as HTMLElement | null;
    const panel = panelRef.current;
    if (panel) (getFocusable(panel)[0] ?? panel).focus();

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        event.stopPropagation();
        onCloseRef.current();
      } else if (panelRef.current) {
        trapTabKey(event, panelRef.current);
      }
    };
    document.addEventListener('keydown', handleKeyDown);
    return () => {
      document.removeEventListener('keydown', handleKeyDown);
      previouslyFocused?.focus?.();
    };
  }, [isOpen]);

  if (!isOpen) return null;

  return (
    <div
      style={{
        position: 'fixed',
        inset: 0,
        backgroundColor: 'rgba(0, 0, 0, 0.7)',
        backdropFilter: 'blur(4px)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
        padding: '16px',
      }}
      onClick={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={title ? titleId : undefined}
        aria-label={title ? undefined : ariaLabel}
        tabIndex={-1}
        style={{
          backgroundColor: colors.bgRaised,
          border: `1px solid ${colors.border}`,
          borderRadius: '12px',
          width: '100%',
          maxWidth: '540px',
          overflow: 'hidden',
          boxShadow: '0 20px 25px -5px rgba(0, 0, 0, 0.5)',
        }}
      >
        {title && (
          <div
            style={{
              padding: '16px 20px',
              borderBottom: `1px solid ${colors.border}`,
              display: 'flex',
              justifyContent: 'space-between',
              alignItems: 'center',
            }}
          >
            <h2 id={titleId} style={{ margin: 0, color: colors.textPrimary, fontSize: '1.1rem' }}>
              {title}
            </h2>
            <button
              type="button"
              onClick={onClose}
              aria-label={t('common.close')}
              style={{
                background: 'transparent',
                border: 'none',
                color: colors.textMuted,
                cursor: 'pointer',
                fontSize: '1.2rem',
                minWidth: 32,
                minHeight: 32,
              }}
            >
              <span aria-hidden="true">✕</span>
            </button>
          </div>
        )}
        <div style={{ padding: '20px' }}>{children}</div>
      </div>
    </div>
  );
};
