import React from 'react';

export type AnomalyCategoryLabel = 'Cosmic' | 'Spatial' | 'Temporal' | 'Energetic' | 'Gravitational';

export interface AnomalyIndicatorProps {
  anomalyId: number;
  anomalyType: string;
  category: AnomalyCategoryLabel;
  confidence: number;
  riskScore: number;
  rewardMultiplierBps: number;
  yieldBonusPct: number;
}

export const AnomalyIndicator: React.FC<AnomalyIndicatorProps> = ({
  anomalyId,
  anomalyType,
  category,
  confidence,
  riskScore,
  rewardMultiplierBps,
  yieldBonusPct,
}) => {
  const riskLevel = riskScore >= 75 ? 'High Risk' : riskScore >= 40 ? 'Moderate Risk' : 'Low Risk';

  return (
    <div role="status" aria-label={`Anomaly #${anomalyId} (${anomalyType}): ${riskLevel}`}>
      <div>
        <strong>#{anomalyId} {anomalyType}</strong> ({category})
      </div>
      <div>
        Confidence: {confidence}% | Risk: {riskScore}/100 ({riskLevel})
      </div>
      <div>
        Reward Multiplier: {(rewardMultiplierBps / 100).toFixed(2)}x | Yield Bonus: +{yieldBonusPct}%
      </div>
    </div>
  );
};
