import React, { useId, useRef, useState } from 'react';
import { colors } from '../../a11y';
import { useI18n } from '../../i18n';

export interface TabItem {
  id: string;
  label: string;
  content: React.ReactNode;
}

export interface TabContainerProps {
  tabs: TabItem[];
  defaultTabId?: string;
  /** Accessible name for the tab list. */
  ariaLabel?: string;
}

/**
 * Tabs (WAI-ARIA tabs pattern, automatic activation): roving tabindex,
 * ArrowLeft/ArrowRight (mirrored in RTL), Home and End.
 */
export const TabContainer: React.FC<TabContainerProps> = ({ tabs, defaultTabId, ariaLabel }) => {
  const { dir } = useI18n();
  const baseId = useId();
  const tabRefs = useRef<Record<string, HTMLButtonElement | null>>({});
  const [activeTab, setActiveTab] = useState<string>(
    defaultTabId || (tabs.length > 0 ? tabs[0].id : '')
  );

  const currentTab = tabs.find((t) => t.id === activeTab);
  const tabId = (id: string) => `${baseId}-tab-${id}`;
  const panelId = (id: string) => `${baseId}-panel-${id}`;

  const activate = (index: number) => {
    const next = tabs[(index + tabs.length) % tabs.length];
    if (!next) return;
    setActiveTab(next.id);
    tabRefs.current[next.id]?.focus();
  };

  const handleKeyDown = (event: React.KeyboardEvent, index: number) => {
    const forward = dir === 'rtl' ? 'ArrowLeft' : 'ArrowRight';
    const backward = dir === 'rtl' ? 'ArrowRight' : 'ArrowLeft';
    let handled = true;
    if (event.key === forward) activate(index + 1);
    else if (event.key === backward) activate(index - 1);
    else if (event.key === 'Home') activate(0);
    else if (event.key === 'End') activate(tabs.length - 1);
    else handled = false;
    if (handled) event.preventDefault();
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', width: '100%' }}>
      <div
        role="tablist"
        aria-label={ariaLabel}
        style={{
          display: 'flex',
          gap: '4px',
          borderBottom: `1px solid ${colors.border}`,
          marginBottom: '16px',
        }}
      >
        {tabs.map((tab, index) => {
          const isActive = tab.id === activeTab;
          return (
            <button
              key={tab.id}
              ref={(el) => {
                tabRefs.current[tab.id] = el;
              }}
              type="button"
              role="tab"
              aria-selected={isActive}
              aria-controls={panelId(tab.id)}
              id={tabId(tab.id)}
              tabIndex={isActive ? 0 : -1}
              onKeyDown={(event) => handleKeyDown(event, index)}
              onClick={() => setActiveTab(tab.id)}
              style={{
                background: 'transparent',
                border: 'none',
                borderBottom: isActive ? `2px solid ${colors.accent}` : '2px solid transparent',
                color: isActive ? colors.accent : colors.textMuted,
                padding: '10px 16px',
                fontSize: '0.9rem',
                fontWeight: 600,
                cursor: 'pointer',
                transition: 'all 0.2s',
              }}
            >
              {tab.label}
            </button>
          );
        })}
      </div>

      {currentTab && (
        <div
          role="tabpanel"
          id={panelId(currentTab.id)}
          aria-labelledby={tabId(currentTab.id)}
          tabIndex={0}
        >
          {currentTab.content}
        </div>
      )}
    </div>
  );
};
