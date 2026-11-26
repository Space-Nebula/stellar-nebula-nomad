import React from 'react';
import { colors } from '../../a11y';
import { useI18n } from '../../i18n';
import { actionButtonStyle } from './actionButtonStyle';

export interface UpgradeButtonProps {
  onUpgrade: () => void | Promise<void>;
  targetLevel: number;
  costDescription?: string;
  isLoading?: boolean;
  disabled?: boolean;
}

export const UpgradeButton: React.FC<UpgradeButtonProps> = ({
  onUpgrade,
  targetLevel,
  costDescription,
  isLoading = false,
  disabled = false,
}) => {
  const { t, formatNumber } = useI18n();
  return (
    <button
      type="button"
      onClick={onUpgrade}
      disabled={disabled || isLoading}
      aria-busy={isLoading}
      // White on amber fails AA (2.1:1); dark text on amber passes (8.3:1).
      style={actionButtonStyle(colors.warningFill, colors.textOnWarning, disabled || isLoading)}
    >
      <span aria-live="polite">
        {isLoading
          ? t('actions.upgrading')
          : t('actions.upgrade', {
              level: formatNumber(targetLevel),
              cost: costDescription ?? t('actions.upgradeDefaultCost'),
            })}
      </span>
    </button>
  );
};
