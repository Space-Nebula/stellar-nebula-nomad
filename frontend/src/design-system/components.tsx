import React from 'react';
import { colors, spacing, typography } from './tokens';

export interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'secondary' | 'outline' | 'danger';
  size?: 'sm' | 'md' | 'lg';
}

export const Button: React.FC<ButtonProps> = ({
  children,
  variant = 'primary',
  size = 'md',
  style,
  disabled,
  ...props
}) => {
  const getVariantStyles = (): React.CSSProperties => {
    switch (variant) {
      case 'secondary':
        return { backgroundColor: colors.secondary, color: colors.text, border: 'none' };
      case 'outline':
        return { backgroundColor: 'transparent', color: colors.primary, border: `1px solid ${colors.primary}` };
      case 'danger':
        return { backgroundColor: colors.danger, color: colors.text, border: 'none' };
      case 'primary':
      default:
        return { backgroundColor: colors.primary, color: '#000000', border: 'none' };
    }
  };

  const getSizeStyles = (): React.CSSProperties => {
    switch (size) {
      case 'sm':
        return { padding: `${spacing.xs} ${spacing.sm}`, fontSize: typography.fontSize.sm };
      case 'lg':
        return { padding: `${spacing.md} ${spacing.xl}`, fontSize: typography.fontSize.lg };
      case 'md':
      default:
        return { padding: `${spacing.sm} ${spacing.md}`, fontSize: typography.fontSize.base };
    }
  };

  return (
    <button
      style={{
        fontFamily: typography.fontFamily.base,
        fontWeight: typography.fontWeight.medium,
        borderRadius: '6px',
        cursor: disabled ? 'not-allowed' : 'pointer',
        opacity: disabled ? 0.6 : 1,
        transition: 'background-color 0.2s ease, opacity 0.2s ease',
        display: 'inline-flex',
        alignItems: 'center',
        justifyContent: 'center',
        ...getVariantStyles(),
        ...getSizeStyles(),
        ...style,
      }}
      disabled={disabled}
      {...props}
    >
      {children}
    </button>
  );
};

export interface CardProps extends React.HTMLAttributes<HTMLDivElement> {
  elevated?: boolean;
}

export const Card: React.FC<CardProps> = ({
  children,
  elevated = false,
  style,
  ...props
}) => (
  <div
    style={{
      backgroundColor: elevated ? colors.surfaceRaised : colors.surface,
      border: `1px solid ${colors.border}`,
      borderRadius: '8px',
      padding: spacing.md,
      color: colors.text,
      fontFamily: typography.fontFamily.base,
      ...style,
    }}
    {...props}
  >
    {children}
  </div>
);
