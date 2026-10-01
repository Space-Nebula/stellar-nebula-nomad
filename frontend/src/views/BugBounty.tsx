import React, { useState } from 'react';

const rewards = [
  { severity: 'Critical', reward: 5000 },
  { severity: 'High', reward: 2000 },
  { severity: 'Medium', reward: 500 },
  { severity: 'Low', reward: 100 },
];

export const BugBounty: React.FC = () => {
  const [severity, setSeverity] = useState('High');
  const selected = rewards.find((tier) => tier.severity === severity);

  return (
    <section style={{ color: '#E2E8F0', display: 'grid', gap: '16px' }}>
      <h2 style={{ margin: 0 }}>Bug Bounty</h2>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(160px, 1fr))', gap: '12px' }}>
        {rewards.map((tier) => (
          <button
            key={tier.severity}
            type="button"
            onClick={() => setSeverity(tier.severity)}
            style={{
              textAlign: 'left',
              padding: '14px',
              borderRadius: '8px',
              border: tier.severity === severity ? '1px solid #38BDF8' : '1px solid #334155',
              background: '#0F172A',
              color: '#E2E8F0',
            }}
          >
            <strong>{tier.severity}</strong>
            <div>{tier.reward.toLocaleString()} reward units</div>
          </button>
        ))}
      </div>
      <form style={{ display: 'grid', gap: '12px', background: '#0F172A', border: '1px solid #334155', borderRadius: '8px', padding: '16px' }}>
        <label>
          Severity
          <select value={severity} onChange={(event) => setSeverity(event.target.value)} style={{ display: 'block', marginTop: '6px' }}>
            {rewards.map((tier) => (
              <option key={tier.severity}>{tier.severity}</option>
            ))}
          </select>
        </label>
        <label>
          Impact summary
          <textarea rows={4} style={{ display: 'block', width: '100%', marginTop: '6px' }} />
        </label>
        <p>Estimated reward: {selected?.reward.toLocaleString()} after reviewer approval and duplicate checks.</p>
      </form>
    </section>
  );
};
