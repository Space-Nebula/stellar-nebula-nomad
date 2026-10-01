import React from 'react';

export interface GovernanceDashboardProps {
  proposalsCreated?: number;
  proposalsPassed?: number;
  votesCast?: number;
  participationBps?: number;
  timelockHours?: number;
}

const panel: React.CSSProperties = {
  background: '#0F172A',
  border: '1px solid #334155',
  borderRadius: '8px',
  padding: '16px',
  color: '#E2E8F0',
};

export const GovernanceDashboard: React.FC<GovernanceDashboardProps> = ({
  proposalsCreated = 18,
  proposalsPassed = 11,
  votesCast = 4280,
  participationBps = 2140,
  timelockHours = 48,
}) => {
  const participation = (participationBps / 100).toFixed(2);

  return (
    <section style={{ display: 'grid', gap: '16px', color: '#E2E8F0' }}>
      <h2 style={{ margin: 0 }}>DAO Governance</h2>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(180px, 1fr))', gap: '12px' }}>
        <div style={panel}>
          <strong>{proposalsCreated}</strong>
          <p>Proposals created</p>
        </div>
        <div style={panel}>
          <strong>{proposalsPassed}</strong>
          <p>Proposals passed</p>
        </div>
        <div style={panel}>
          <strong>{votesCast.toLocaleString()}</strong>
          <p>Votes cast</p>
        </div>
        <div style={panel}>
          <strong>{participation}%</strong>
          <p>Participation</p>
        </div>
      </div>
      <div style={panel}>
        <h3 style={{ marginTop: 0 }}>Execution policy</h3>
        <p>Parameter and feature proposals execute automatically after passing.</p>
        <p>Treasury spends require multisig approval. Contract upgrades require manual execution.</p>
        <p>All passing proposals wait {timelockHours} hours before execution.</p>
      </div>
    </section>
  );
};
