import React from 'react';
import { colors } from '../../a11y';
import { useI18n } from '../../i18n';
import { actionButtonStyle } from './actionButtonStyle';

export interface TradeButtonProps {
  onTrade: () => void | Promise<void>;
  tradeType: 'buy' | 'sell';
  price: string;
  currency?: string;
  isLoading?: boolean;
  disabled?: boolean;
}

export const TradeButton: React.FC<TradeButtonProps> = ({
  onTrade,
  tradeType,
  price,
  currency = 'XLM',
  isLoading = false,
  disabled = false,
}) => {
  const { t } = useI18n();
  const isBuy = tradeType === 'buy';

  let label: string;
  if (isLoading) label = t(isBuy ? 'actions.processingBuy' : 'actions.processingSell');
  else label = t(isBuy ? 'actions.buy' : 'actions.list', { price, currency });

  return (
    <button
      type="button"
      onClick={onTrade}
      disabled={disabled || isLoading}
      aria-busy={isLoading}
      style={actionButtonStyle(
        isBuy ? colors.primaryFill : colors.secondaryFill,
        colors.textOnAccent,
        disabled || isLoading
      )}
    >
      <span aria-live="polite">{label}</span>
    </button>
  );
};
