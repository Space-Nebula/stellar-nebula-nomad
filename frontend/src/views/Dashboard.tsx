import React from 'react';
import { Card, Button, spacing, colors } from '../design-system';

export interface PlayerStats {
  level: number;
  reputation: number;
  explorationsCount: number;
  winRate: string;
}

export interface PlayerMission {
  id: string;
  title: string;
  progress: number; // 0 - 100
  reward: string;
}

export interface PlayerHolding {
  symbol: string;
  amount: string;
  usdValue: string;
}

export interface DashboardProps {
  playerName?: string;
  stats?: PlayerStats;
  missions?: PlayerMission[];
  holdings?: PlayerHolding[];
  recentActivities?: string[];
  onExploreClick?: () => void;
}

export const Dashboard: React.FC<DashboardProps> = ({
  playerName = 'Nomad Pilot',
  stats = {
    level: 12,
    reputation: 840,
    explorationsCount: 142,
    winRate: '78.5%',
  },
  missions = [
    { id: 'm1', title: 'Survey Orion Nebula Perimeter', progress: 65, reward: '500 LUMEN' },
    { id: 'm2', title: 'Harvest 10 Void Crystal Samples', progress: 90, reward: '250 LUMEN' },
  ],
  holdings = [
    { symbol: 'XLM', amount: '1,450.00', usdValue: '$188.50' },
    { symbol: 'NOMAD', amount: '12,000.00', usdValue: '$600.00' },
  ],
  recentActivities = [
    'Completed deep scan of Sector 7-Alpha',
    'Traded 50 Titanium Alloys at Outer Rim Station',
    'Received alliance commendation from Starfleet Omega',
  ],
  onExploreClick,
}) => {
  return (
    <div style={{ padding: spacing.lg, maxWidth: '1200px', margin: '0 auto' }}>
      <header style={{ marginBottom: spacing.lg, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <h1 style={{ margin: 0, fontSize: '1.75rem', color: colors.text }}>Pilot Dashboard</h1>
          <p style={{ margin: `${spacing.xs} 0 0 0`, color: colors.textMuted }}>Welcome back, Commander {playerName}</p>
        </div>
        {onExploreClick && (
          <Button variant="primary" onClick={onExploreClick}>
            Launch Exploration
          </Button>
        )}
      </header>

      {/* Stats Grid */}
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: spacing.md, marginBottom: spacing.lg }}>
        <Card>
          <div style={{ fontSize: '0.875rem', color: colors.textMuted }}>Pilot Level</div>
          <div style={{ fontSize: '1.5rem', fontWeight: 'bold', color: colors.primary, marginTop: spacing.xs }}>{stats.level}</div>
        </Card>
        <Card>
          <div style={{ fontSize: '0.875rem', color: colors.textMuted }}>Reputation Score</div>
          <div style={{ fontSize: '1.5rem', fontWeight: 'bold', color: colors.success, marginTop: spacing.xs }}>{stats.reputation}</div>
        </Card>
        <Card>
          <div style={{ fontSize: '0.875rem', color: colors.textMuted }}>Total Explorations</div>
          <div style={{ fontSize: '1.5rem', fontWeight: 'bold', color: colors.secondary, marginTop: spacing.xs }}>{stats.explorationsCount}</div>
        </Card>
        <Card>
          <div style={{ fontSize: '0.875rem', color: colors.textMuted }}>Mission Win Rate</div>
          <div style={{ fontSize: '1.5rem', fontWeight: 'bold', color: colors.warning, marginTop: spacing.xs }}>{stats.winRate}</div>
        </Card>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(320px, 1fr))', gap: spacing.lg }}>
        {/* Active Missions */}
        <Card>
          <h2 style={{ fontSize: '1.125rem', marginTop: 0, marginBottom: spacing.md, color: colors.text }}>Active Missions</h2>
          <div style={{ display: 'flex', flexDirection: 'column', gap: spacing.sm }}>
            {missions.map((m) => (
              <div key={m.id} style={{ borderBottom: `1px solid ${colors.border}`, paddingBottom: spacing.sm }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.9rem', marginBottom: spacing.xs }}>
                  <span style={{ fontWeight: 500 }}>{m.title}</span>
                  <span style={{ color: colors.primary }}>{m.reward}</span>
                </div>
                <div style={{ height: '6px', background: colors.surfaceRaised, borderRadius: '3px', overflow: 'hidden' }}>
                  <div style={{ width: `${m.progress}%`, height: '100%', background: colors.primary }} />
                </div>
              </div>
            ))}
          </div>
        </Card>

        {/* Asset Holdings */}
        <Card>
          <h2 style={{ fontSize: '1.125rem', marginTop: 0, marginBottom: spacing.md, color: colors.text }}>Asset Holdings</h2>
          <div style={{ display: 'flex', flexDirection: 'column', gap: spacing.sm }}>
            {holdings.map((h) => (
              <div key={h.symbol} style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', padding: `${spacing.xs} 0` }}>
                <div>
                  <div style={{ fontWeight: 600 }}>{h.symbol}</div>
                  <div style={{ fontSize: '0.8rem', color: colors.textMuted }}>{h.amount}</div>
                </div>
                <div style={{ fontWeight: 600, color: colors.success }}>{h.usdValue}</div>
              </div>
            ))}
          </div>
        </Card>

        {/* Activity Feed */}
        <Card style={{ gridColumn: '1 / -1' }}>
          <h2 style={{ fontSize: '1.125rem', marginTop: 0, marginBottom: spacing.md, color: colors.text }}>Recent Activity</h2>
          <ul style={{ margin: 0, paddingLeft: spacing.md, color: colors.textMuted }}>
            {recentActivities.map((act, idx) => (
              <li key={idx} style={{ marginBottom: spacing.xs }}>{act}</li>
            ))}
          </ul>
        </Card>
      </div>
    </div>
  );
};
