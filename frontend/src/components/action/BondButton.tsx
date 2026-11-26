import React from 'react';
import { colors } from '../../a11y';
import { useI18n } from '../../i18n';
import { actionButtonStyle } from './actionButtonStyle';

export interface BondButtonProps {
  onBond: () => void | Promise<void>;
  stakeAmount: string;
  guildName: string;
  isLoading?: boolean;
  disabled?: boolean;
}

export const BondButton: React.FC<BondButtonProps> = ({
  onBond,
  stakeAmount,
  guildName,
  isLoading = false,
  disabled = false,
}) => {
  const { t } = useI18n();
  return (
    <button
      type="button"
      onClick={onBond}
      disabled={disabled || isLoading}
      aria-busy={isLoading}
      style={actionButtonStyle(colors.bondFill, colors.textOnAccent, disabled || isLoading)}
    >
      <span aria-live="polite">
        {isLoading
          ? t('actions.bonding')
          : t('actions.bond', { amount: stakeAmount, guild: guildName })}
      </span>
    </button>
  );
};
