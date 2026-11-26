import React, { Component, ErrorInfo, ReactNode } from 'react';
import { colors } from '../../a11y';
import { i18n } from '../../i18n';

export interface ErrorBoundaryProps {
  children: ReactNode;
  fallback?: ReactNode;
  onError?: (error: Error, info: ErrorInfo) => void;
}

interface State {
  hasError: boolean;
  error: Error | null;
}

export class ErrorBoundary extends Component<ErrorBoundaryProps, State> {
  public override state: State = {
    hasError: false,
    error: null,
  };

  public static getDerivedStateFromError(error: Error): State {
    return { hasError: true, error };
  }

  public override componentDidCatch(error: Error, info: ErrorInfo): void {
    this.props.onError?.(error, info);
  }

  public reset = (): void => {
    this.setState({ hasError: false, error: null });
  };

  public override render(): ReactNode {
    if (this.state.hasError) {
      if (this.props.fallback) {
        return this.props.fallback;
      }

      // Class components cannot use hooks; read the shared i18n instance.
      const t = i18n.t.bind(i18n);
      return (
        <div
          role="alert"
          style={{
            padding: '24px',
            backgroundColor: colors.bgRaised,
            border: `1px solid ${colors.danger}`,
            borderRadius: '8px',
            color: colors.textPrimary,
            textAlign: 'center',
            margin: '16px 0',
          }}
        >
          <h2 style={{ color: colors.danger, marginTop: 0, fontSize: '1.1rem' }}>
            {t('errors.title')}
          </h2>
          <p style={{ color: colors.textMuted, fontSize: '0.875rem' }}>
            {this.state.error?.message || t('errors.unexpected')}
          </p>
          <button
            type="button"
            onClick={this.reset}
            style={{
              backgroundColor: colors.primaryFill,
              color: colors.textOnAccent,
              border: 'none',
              padding: '8px 16px',
              borderRadius: '6px',
              cursor: 'pointer',
              fontWeight: 600,
            }}
          >
            {t('common.tryAgain')}
          </button>
        </div>
      );
    }

    return this.props.children;
  }
}
