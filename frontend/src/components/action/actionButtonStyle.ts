import type React from 'react';

/** Shared style for the action buttons; `background`/`color` come from AA-checked tokens. */
export const actionButtonStyle = (
  background: string,
  color: string,
  inactive: boolean
): React.CSSProperties => ({
  backgroundColor: background,
  color,
  border: 'none',
  borderRadius: '8px',
  padding: '10px 18px',
  fontSize: '0.9rem',
  fontWeight: 700,
  cursor: inactive ? 'not-allowed' : 'pointer',
  opacity: inactive ? 0.6 : 1,
  display: 'inline-flex',
  alignItems: 'center',
  justifyContent: 'center',
  gap: '6px',
});
