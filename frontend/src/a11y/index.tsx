/**
 * Accessibility primitives shared by the component library (Issue #698).
 */
import React from 'react';

// ── Colour tokens ──────────────────────────────────────────────────────────
//
// Every text/background pair used by the components is checked against WCAG
// 2.1 AA in `src/tests/accessibility.test.tsx` (4.5:1 for text, 3:1 for
// non-text UI such as borders and icons). Use these tokens instead of raw hex
// values so new pairs are covered.

export const colors = {
  // Surfaces
  bgApp: '#0B1020',
  bgSurface: '#0F172A',
  bgRaised: '#1E293B',
  border: '#334155',
  // Text
  textPrimary: '#F8FAFC',
  textSecondary: '#CBD5E1',
  textMuted: '#94A3B8',
  textOnAccent: '#FFFFFF',
  textOnWarning: '#0F172A',
  // Accents (as text on dark surfaces)
  accent: '#38BDF8',
  success: '#34D399',
  danger: '#F87171',
  warning: '#FBBF24',
  epic: '#C084FC',
  // Accents (as backgrounds behind white text)
  primaryFill: '#2563EB',
  secondaryFill: '#6D28D9',
  successFill: '#047857',
  dangerFill: '#DC2626',
  warningFill: '#F59E0B',
  bondFill: '#BE185D',
  selectedFill: '#4F46E5',
  // Focus ring
  focus: '#38BDF8',
} as const;

/** Relative luminance of a `#RRGGBB` colour (WCAG 2.1 definition). */
export const relativeLuminance = (hex: string): number => {
  const value = hex.replace('#', '');
  const channels = [0, 2, 4].map((i) => parseInt(value.slice(i, i + 2), 16) / 255);
  const [r, g, b] = channels.map((c) =>
    c <= 0.03928 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4)
  );
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};

/** WCAG contrast ratio between two `#RRGGBB` colours (1–21). */
export const contrastRatio = (foreground: string, background: string): number => {
  const a = relativeLuminance(foreground);
  const b = relativeLuminance(background);
  const [light, dark] = a > b ? [a, b] : [b, a];
  return (light + 0.05) / (dark + 0.05);
};

// ── Screen-reader-only content ─────────────────────────────────────────────

/** Inline style that hides content visually but keeps it for screen readers. */
export const visuallyHidden: React.CSSProperties = {
  position: 'absolute',
  width: 1,
  height: 1,
  padding: 0,
  margin: -1,
  overflow: 'hidden',
  clip: 'rect(0, 0, 0, 0)',
  whiteSpace: 'nowrap',
  border: 0,
};

export const VisuallyHidden: React.FC<React.PropsWithChildren<{ id?: string }>> = ({
  id,
  children,
}) => (
  <span id={id} style={visuallyHidden}>
    {children}
  </span>
);

// ── Focus management ───────────────────────────────────────────────────────

export const FOCUSABLE_SELECTOR = [
  'a[href]',
  'button:not([disabled])',
  'input:not([disabled]):not([type="hidden"])',
  'select:not([disabled])',
  'textarea:not([disabled])',
  '[tabindex]:not([tabindex="-1"])',
].join(',');

export const getFocusable = (root: HTMLElement): HTMLElement[] =>
  Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE_SELECTOR));

/**
 * Keep Tab / Shift+Tab inside `root` (for modal dialogs). Returns true when
 * the event was handled.
 */
export const trapTabKey = (event: KeyboardEvent | React.KeyboardEvent, root: HTMLElement): boolean => {
  if (event.key !== 'Tab') return false;
  const focusable = getFocusable(root);
  if (focusable.length === 0) {
    event.preventDefault();
    root.focus();
    return true;
  }
  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  const active = document.activeElement;
  if (event.shiftKey && (active === first || active === root)) {
    event.preventDefault();
    last.focus();
    return true;
  }
  if (!event.shiftKey && active === last) {
    event.preventDefault();
    first.focus();
    return true;
  }
  return false;
};

// ── Global styles ──────────────────────────────────────────────────────────

/**
 * Global accessibility CSS: a visible focus ring for keyboard users, a
 * skip-link that appears on focus, and reduced motion. Inline styles cannot
 * express `:focus-visible` or media queries, so `DashboardLayout` renders this
 * once; apps that do not use the layout can render `<AccessibilityStyles />`
 * or import `styles/accessibility.css`.
 */
export const ACCESSIBILITY_CSS = `
:focus-visible {
  outline: 3px solid ${colors.focus};
  outline-offset: 2px;
}
.snn-skip-link {
  position: absolute;
  inset-inline-start: 8px;
  top: -100px;
  z-index: 2000;
  padding: 8px 16px;
  border-radius: 6px;
  background: ${colors.textPrimary};
  color: ${colors.bgSurface};
  font-weight: 700;
  text-decoration: none;
}
.snn-skip-link:focus {
  top: 8px;
}
.snn-reveal-on-focus:not(:focus) {
  width: 1px !important;
  height: 1px !important;
  padding: 0 !important;
  margin: -1px !important;
  overflow: hidden !important;
  clip: rect(0, 0, 0, 0) !important;
  border: 0 !important;
}
@media (prefers-reduced-motion: reduce) {
  *, *::before, *::after {
    animation-duration: 0.01ms !important;
    animation-iteration-count: 1 !important;
    scroll-behavior: auto !important;
    transition-duration: 0.01ms !important;
  }
}
`;

export const AccessibilityStyles: React.FC = () => (
  <style data-snn-a11y="">{ACCESSIBILITY_CSS}</style>
);
