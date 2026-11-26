import React from 'react';
import { colors } from '../../a11y';
import { useI18n } from '../../i18n';
import { actionButtonStyle } from './actionButtonStyle';

export interface MintButtonProps {
  onMint: () => void | Promise<void>;
  isLoading?: boolean;
  costXlm?: number;
  disabled?: boolean;
}

export const MintButton: React.FC<MintButtonProps> = ({
  onMint,
  isLoading = false,
  costXlm = 50,
  disabled = false,
}) => {
  const { t, formatNumber } = useI18n();
  return (
    <button
      type="button"
      onClick={onMint}
      disabled={disabled || isLoading}
      aria-busy={isLoading}
      style={actionButtonStyle(colors.successFill, colors.textOnAccent, disabled || isLoading)}
    >
      <span aria-live="polite">
        {isLoading ? t('actions.minting') : t('actions.mint', { cost: formatNumber(costXlm) })}
      </span>
    </button>
  );
};
