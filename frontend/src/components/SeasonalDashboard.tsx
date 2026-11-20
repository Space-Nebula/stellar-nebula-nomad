import React from 'react';

/** Mirrors `seasons::SeasonTheme` (Issue #594). */
export type SeasonTheme =
  | 'EmberNebula'
  | 'VoidTide'
  | 'StellarApex'
  | 'CrimsonDrift'
  | 'AzureVeil'
  | 'NovaBurst';

export type PassTrack = 'free' | 'premium';

export interface SeasonInfoView {
  id: number;
  title: string;
  startSecs: number;
  endSecs: number;
  /** 1–3; each chapter is 30 days. */
  chapter: number;
  theme: SeasonTheme;
  nebulaType: string;
}

export interface SeasonPassView {
  xp: number;
  freeTier: number;
  premiumTier: number;
  hasPremium: boolean;
  nextFreeTier?: number;
  nextFreeTierXp?: number;
}

export interface ExclusiveContentView {
  cosmeticId: number;
  themeSymbol: string;
  eventRewardMultiplierBps: number;
  featuredTrack: PassTrack;
}

export interface SeasonalChallengeView {
  id: number;
  title: string;
  metric: string;
  target: number;
  current: number;
  completed: boolean;
  claimed: boolean;
  reward: number;
  endsAtSecs: number;
}

export interface SeasonalDashboardProps {
  season: SeasonInfoView;
  pass?: SeasonPassView;
  exclusive: ExclusiveContentView;
  challenges: SeasonalChallengeView[];
  /** Ledger timestamp in seconds. */
  nowSecs: number;
  onClaimPassReward?: (tier: number, track: PassTrack) => void;
  onClaimChallengeReward?: (challengeId: number) => void;
}

export const SEASON_DURATION_SECS = 90 * 24 * 60 * 60;
export const CHAPTER_DURATION_SECS = 30 * 24 * 60 * 60;
export const MAX_FREE_TIERS = 30;
export const MAX_PREMIUM_TIERS = 50;

const THEME_ORDER: SeasonTheme[] = [
  'EmberNebula',
  'VoidTide',
  'StellarApex',
  'CrimsonDrift',
  'AzureVeil',
  'NovaBurst',
];

const THEME_COLOURS: Record<SeasonTheme, string> = {
  EmberNebula: '#F97316',
  VoidTide: '#8B5CF6',
  StellarApex: '#38BDF8',
  CrimsonDrift: '#EF4444',
  AzureVeil: '#60A5FA',
  NovaBurst: '#FBBF24',
};

/** Pure mirror of `SeasonTheme::from_season_id`. */
export function themeForSeason(seasonId: number): SeasonTheme {
  const safeId = Number.isFinite(seasonId) ? Math.max(1, Math.floor(seasonId)) : 1;
  const index = (safeId - 1) % THEME_ORDER.length;
  return THEME_ORDER[index];
}

/** XP threshold for a pass tier (matches the on-chain default tables). */
export function tierXpRequired(tier: number, track: PassTrack): number {
  const max = track === 'free' ? MAX_FREE_TIERS : MAX_PREMIUM_TIERS;
  if (!Number.isFinite(tier) || tier < 1 || tier > max) return 0;
  const base = track === 'free' ? 100 : 200;
  const step = track === 'free' ? 150 : 100;
  return base + (Math.floor(tier) - 1) * step;
}

/** Highest fully unlocked tier for an XP total. */
export function highestUnlockedTier(xp: number, track: PassTrack): number {
  const max = track === 'free' ? MAX_FREE_TIERS : MAX_PREMIUM_TIERS;
  const base = track === 'free' ? 100 : 200;
  const step = track === 'free' ? 150 : 100;
  if (!Number.isFinite(xp) || xp < base) return 0;
  return Math.min(max, 1 + Math.floor((xp - base) / step));
}

/** Compact human-readable countdown for ledger timestamps. */
export function formatCountdown(seconds: number): string {
  const total = Number.isFinite(seconds) ? Math.max(0, Math.floor(seconds)) : 0;
  const days = Math.floor(total / 86400);
  const hours = Math.floor((total % 86400) / 3600);
  if (days > 0) return `${days}d ${hours}h`;
  const minutes = Math.floor((total % 3600) / 60);
  return hours > 0 ? `${hours}h ${minutes}m` : `${minutes}m`;
}

const pct = (value: number, total: number): number =>
  total <= 0 ? 0 : Math.max(0, Math.min(100, (value / total) * 100));

const card: React.CSSProperties = {
  background: '#0F172A',
  color: '#E2E8F0',
  border: '1px solid #334155',
  borderRadius: 8,
  padding: 16,
};

const button: React.CSSProperties = {
  background: '#2563EB',
  color: '#F8FAFC',
  border: 'none',
  borderRadius: 6,
  padding: '4px 10px',
  cursor: 'pointer',
};

/** Season header, pass tracks, exclusive content and challenges (#594). */
export const SeasonalDashboard: React.FC<SeasonalDashboardProps> = ({
  season,
  pass,
  exclusive,
  challenges,
  nowSecs,
  onClaimPassReward,
  onClaimChallengeReward,
}) => {
  const theme = season.theme || themeForSeason(season.id);
  const colour = THEME_COLOURS[theme] || '#94A3B8';
  const remaining = Math.max(0, season.endSecs - nowSecs);
  const elapsed = Math.max(0, Math.min(SEASON_DURATION_SECS, nowSecs - season.startSecs));
  const progress = pct(elapsed, SEASON_DURATION_SECS);
  const freeTier = pass?.freeTier ?? highestUnlockedTier(pass?.xp ?? 0, 'free');
  const premiumTier = pass?.premiumTier ?? highestUnlockedTier(pass?.xp ?? 0, 'premium');
  const nextFreeTier = pass?.nextFreeTier ?? freeTier + 1;
  const nextFreeTierXp =
    pass?.nextFreeTierXp ??
    (nextFreeTier <= MAX_FREE_TIERS ? tierXpRequired(nextFreeTier, 'free') : undefined);

  return (
    <section aria-label="Seasonal dashboard" data-testid="seasonal-dashboard" style={{ display: 'grid', gap: 16 }}>
      <div style={{ ...card, borderColor: colour }}>
        <header style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'baseline' }}>
          <h2 data-testid="season-title" style={{ margin: 0 }}>
            Season {season.id}: {season.title}
          </h2>
          <span data-testid="season-theme" style={{ color: colour, fontWeight: 700 }}>
            {theme}
          </span>
        </header>
        <p style={{ color: '#94A3B8', margin: '8px 0' }}>
          Chapter {season.chapter}/3 · {season.nebulaType} · ends in {formatCountdown(remaining)}
        </p>
        <div
          role="progressbar"
          aria-valuenow={Math.round(progress)}
          aria-valuemin={0}
          aria-valuemax={100}
          data-testid="season-progress"
          style={{ height: 8, background: '#1E293B', borderRadius: 4, overflow: 'hidden' }}
        >
          <div style={{ width: `${progress}%`, height: '100%', background: colour }} />
        </div>
        <small style={{ color: '#64748B' }}>
          90-day cycle · next season starts when the current window closes
        </small>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))', gap: 16 }}>
        <div style={card} data-testid="season-pass">
          <h3 style={{ marginTop: 0 }}>Season pass</h3>
          <p data-testid="pass-xp" style={{ margin: '4px 0' }}>
            {pass?.xp ?? 0} XP
          </p>
          <p style={{ margin: '4px 0' }}>
            Free tier <strong data-testid="free-tier">{freeTier}</strong>/{MAX_FREE_TIERS} · Premium tier{' '}
            <strong data-testid="premium-tier">{premiumTier}</strong>/{MAX_PREMIUM_TIERS}
          </p>
          {nextFreeTierXp !== undefined && (
            <small style={{ color: '#94A3B8' }}>
              Next free tier {nextFreeTier} at {nextFreeTierXp} XP
            </small>
          )}
          <p style={{ margin: '8px 0 0' }}>
            <span data-testid="premium-status">
              {pass?.hasPremium ? 'Premium unlocked (cosmetics)' : 'Free track'}
            </span>
            {onClaimPassReward && freeTier > 0 && (
              <button type="button" style={{ ...button, marginLeft: 8 }} onClick={() => onClaimPassReward(freeTier, 'free')}>
                Claim free
              </button>
            )}
            {onClaimPassReward && pass?.hasPremium && premiumTier > 0 && (
              <button type="button" style={{ ...button, marginLeft: 8 }} onClick={() => onClaimPassReward(premiumTier, 'premium')}>
                Claim premium
              </button>
            )}
          </p>
        </div>

        <div style={card} data-testid="exclusive-content">
          <h3 style={{ marginTop: 0 }}>Exclusive this season</h3>
          <p style={{ margin: '4px 0' }}>
            {exclusive.themeSymbol} palette · cosmetic #{exclusive.cosmeticId}
          </p>
          <p style={{ margin: '4px 0', color: '#94A3B8' }}>
            Event pools ×{(exclusive.eventRewardMultiplierBps / 10_000).toFixed(2)} · featured{' '}
            {exclusive.featuredTrack} track
          </p>
          <small style={{ color: '#64748B' }}>Rotates every six seasons; never reissued.</small>
        </div>
      </div>

      <div style={card}>
        <h3 style={{ marginTop: 0 }}>Seasonal challenges</h3>
        {challenges.length === 0 ? (
          <p data-testid="no-challenges" style={{ color: '#94A3B8' }}>
            No active challenges right now.
          </p>
        ) : (
          <ul data-testid="challenge-list" style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 10 }}>
            {challenges.map((challenge) => {
              const completed = challenge.completed || challenge.current >= challenge.target;
              return (
                <li key={challenge.id} data-testid={`challenge-${challenge.id}`}>
                  <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                    <span>{challenge.title}</span>
                    <small style={{ color: '#94A3B8' }}>
                      {Math.min(challenge.current, challenge.target)}/{challenge.target} {challenge.metric}
                    </small>
                  </div>
                  <div style={{ height: 6, background: '#1E293B', borderRadius: 3, margin: '4px 0' }}>
                    <div
                      style={{
                        width: `${pct(challenge.current, challenge.target)}%`,
                        height: '100%',
                        background: completed ? '#10B981' : colour,
                      }}
                    />
                  </div>
                  <small style={{ color: '#64748B' }}>
                    {formatCountdown(challenge.endsAtSecs - nowSecs)} left · reward {challenge.reward}
                    {onClaimChallengeReward && completed && !challenge.claimed && (
                      <button
                        type="button"
                        style={{ ...button, marginLeft: 8 }}
                        onClick={() => onClaimChallengeReward(challenge.id)}
                      >
                        Claim
                      </button>
                    )}
                  </small>
                </li>
              );
            })}
          </ul>
        )}
      </div>
    </section>
  );
};

export default SeasonalDashboard;
