import React from 'react';

export interface ReputationProps {
  score?: number;
  positiveActions?: number;
  negativeActions?: number;
}

function level(score: number): string {
  if (score <= 20) return 'Newcomer';
  if (score <= 40) return 'Scout';
  if (score <= 65) return 'Contributor';
  if (score <= 85) return 'Veteran';
  return 'Legend';
}

export const Reputation: React.FC<ReputationProps> = ({
  score = 72,
  positiveActions = 34,
  negativeActions = 2,
}) => {
  const currentLevel = level(score);

  return (
    <section style={{ color: '#E2E8F0', display: 'grid', gap: '16px' }}>
      <h2 style={{ margin: 0 }}>Reputation</h2>
      <div style={{ background: '#0F172A', border: '1px solid #334155', borderRadius: '8px', padding: '16px' }}>
        <strong style={{ fontSize: '2rem' }}>{score}</strong>
        <p>{currentLevel}</p>
        <progress value={score} max={100} style={{ width: '100%' }} />
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))', gap: '12px' }}>
        <div style={{ background: '#0F172A', border: '1px solid #334155', borderRadius: '8px', padding: '14px' }}>
          <strong>{positiveActions}</strong>
          <p>Positive actions</p>
        </div>
        <div style={{ background: '#0F172A', border: '1px solid #334155', borderRadius: '8px', padding: '14px' }}>
          <strong>{negativeActions}</strong>
          <p>Negative actions</p>
        </div>
      </div>
    </section>
  );
};
