import React from 'react';
import { AccessibilityStyles, colors } from '../../a11y';
import { useI18n } from '../../i18n';

export interface DashboardLayoutProps {
  header: React.ReactNode;
  sidebar?: React.ReactNode;
  children: React.ReactNode;
}

/**
 * Page shell with landmarks (banner, navigation, main), a skip link to the
 * main content, and the global accessibility styles (focus ring, reduced
 * motion).
 */
export const DashboardLayout: React.FC<DashboardLayoutProps> = ({ header, sidebar, children }) => {
  const { t, dir } = useI18n();
  return (
    <div
      dir={dir}
      style={{
        display: 'flex',
        flexDirection: 'column',
        minHeight: '100vh',
        backgroundColor: colors.bgApp,
        color: colors.textPrimary,
        fontFamily: 'Inter, system-ui, sans-serif',
      }}
    >
      <AccessibilityStyles />
      <a className="snn-skip-link" href="#main-content">
        {t('common.skipToContent')}
      </a>
      <header
        style={{
          borderBottom: `1px solid ${colors.bgRaised}`,
          padding: '12px 24px',
          backgroundColor: colors.bgSurface,
          position: 'sticky',
          top: 0,
          zIndex: 40,
        }}
      >
        {header}
      </header>

      <div style={{ display: 'flex', flex: 1 }}>
        {sidebar && (
          <nav
            aria-label={t('layout.primaryNavigation')}
            style={{
              width: '260px',
              borderInlineEnd: `1px solid ${colors.bgRaised}`,
              backgroundColor: colors.bgSurface,
              padding: '20px 16px',
            }}
          >
            {sidebar}
          </nav>
        )}
        <main
          id="main-content"
          tabIndex={-1}
          style={{ flex: 1, padding: '24px 20px', overflowY: 'auto' }}
        >
          {children}
        </main>
      </div>
    </div>
  );
};
