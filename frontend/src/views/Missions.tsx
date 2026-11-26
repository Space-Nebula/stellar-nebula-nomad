import React from 'react';

export interface MissionViewItem {
  missionId: number;
  title?: string;
  description?: string;
  missionType: string;
  targetCount: number;
  currentProgress: number;
  reward: string;
  narrativeTier?: string;
  archetypeTag?: string;
  completed: boolean;
  claimed: boolean;
}

export interface MissionsProps {
  missions?: MissionViewItem[];
  onGenerateAiMission?: () => void;
  onClaimMission?: (missionId: number) => void;
}

export const Missions: React.FC<MissionsProps> = ({
  missions = [],
  onGenerateAiMission,
  onClaimMission,
}) => {
  return (
    <section aria-label="AI-Powered Dynamic Missions">
      <header>
        <h1>AI-Powered Dynamic Missions</h1>
        <p>Missions generated procedurally based on your playstyle, archetype, and seasonal tier.</p>
        {onGenerateAiMission && (
          <button type="button" onClick={onGenerateAiMission}>
            Generate AI Mission
          </button>
        )}
      </header>

      <ul>
        {missions.map(m => {
          const pct = m.targetCount > 0 ? Math.min(100, Math.round((m.currentProgress / m.targetCount) * 100)) : 0;
          return (
            <li key={m.missionId}>
              <h3>{m.title ?? `Mission #${m.missionId} (${m.missionType})`}</h3>
              {m.description && <p>{m.description}</p>}
              <div>
                <span>Type: {m.missionType}</span>
                {m.archetypeTag && <span> | Archetype: {m.archetypeTag}</span>}
                {m.narrativeTier && <span> | Tier: {m.narrativeTier}</span>}
              </div>
              <div>
                Progress: {m.currentProgress} / {m.targetCount} ({pct}%) — Reward: {m.reward}
              </div>
              {m.completed && !m.claimed && onClaimMission && (
                <button type="button" onClick={() => onClaimMission(m.missionId)}>
                  Claim Reward
                </button>
              )}
            </li>
          );
        })}
      </ul>
    </section>
  );
};
