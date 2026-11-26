import React, { useCallback, useEffect, useMemo, useState } from 'react';

export type GuildQuestFrequency = 'oneOff' | 'daily' | 'weekly' | 'seasonal';
export type GuildQuestStatus = 'active' | 'completed' | 'expired';
export type GuildQuestFilter = 'all' | GuildQuestStatus;

/** View model matching `GuildQuestSummary` from the contract. */
export interface GuildQuestView {
  questId: string | number;
  allianceId?: string | number;
  questType: string;
  targetCount: number;
  currentProgress: number;
  status?: GuildQuestStatus;
  frequency?: GuildQuestFrequency;
  tier?: number;
  chainId?: string | number | null;
  minContributors?: number;
  contributors?: number;
  assignees?: number;
  rewardEssence?: string | number;
  rewardXp?: number;
  expiresAt?: number;
}

export interface GuildQuestBoardProps {
  allianceId: string | number;
  /** Pre-loaded quests; when omitted, `loadQuests` is used. */
  quests?: GuildQuestView[];
  loadQuests?: (allianceId: string | number) => Promise<GuildQuestView[]>;
  onContribute?: (questId: string | number, amount: number) => Promise<void> | void;
  onClaim?: (questId: string | number) => Promise<void> | void;
}

const STATUS_COLORS: Record<GuildQuestStatus, { background: string; color: string }> = {
  active: { background: 'rgba(56, 189, 248, 0.15)', color: '#38BDF8' },
  completed: { background: 'rgba(16, 185, 129, 0.15)', color: '#10B981' },
  expired: { background: 'rgba(239, 68, 68, 0.15)', color: '#EF4444' },
};

const FREQUENCY_LABELS: Record<GuildQuestFrequency, string> = {
  oneOff: 'One-off',
  daily: 'Daily',
  weekly: 'Weekly',
  seasonal: 'Seasonal',
};

const FILTERS: Array<{ value: GuildQuestFilter; label: string }> = [
  { value: 'all', label: 'All' },
  { value: 'active', label: 'Active' },
  { value: 'completed', label: 'Completed' },
  { value: 'expired', label: 'Expired' },
];

function effectiveStatus(quest: GuildQuestView, nowSecs: number): GuildQuestStatus {
  if (quest.status) return quest.status;
  if (quest.expiresAt && nowSecs > quest.expiresAt) return 'expired';
  return 'active';
}

function formatAmount(value?: string | number): string {
  if (value === undefined || value === null) return '0';
  const numeric = typeof value === 'number' ? value : Number(value);
  return Number.isFinite(numeric) ? numeric.toLocaleString() : String(value);
}

function remainingTime(expiresAt: number | undefined, nowSecs: number): string {
  if (!expiresAt) return 'No deadline';
  const remaining = Math.max(0, expiresAt - nowSecs);
  if (remaining === 0) return 'Expired';
  if (remaining < 3600) return `${Math.ceil(remaining / 60)}m left`;
  if (remaining < 86400) return `${Math.floor(remaining / 3600)}h left`;
  return `${Math.floor(remaining / 86400)}d left`;
}

export const GuildQuestBoard: React.FC<GuildQuestBoardProps> = ({
  allianceId,
  quests,
  loadQuests,
  onContribute,
  onClaim,
}) => {
  const [items, setItems] = useState<GuildQuestView[]>(quests ?? []);
  const [filter, setFilter] = useState<GuildQuestFilter>('all');
  const [amounts, setAmounts] = useState<Record<string, number>>({});
  const [busy, setBusy] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    if (!loadQuests) return;
    try {
      setError(null);
      setItems(await loadQuests(allianceId));
    } catch (err) {
      setError(err instanceof Error ? err.message : 'Unable to load guild quests');
    }
  }, [allianceId, loadQuests]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    if (quests) setItems(quests);
  }, [quests]);

  const nowSecs = Math.floor(Date.now() / 1000);
  const visible = useMemo(
    () => items.filter((quest) => filter === 'all' || effectiveStatus(quest, nowSecs) === filter),
    [items, filter, nowSecs],
  );

  const handleContribute = useCallback(
    async (quest: GuildQuestView) => {
      if (!onContribute) return;
      const id = String(quest.questId);
      const amount = Math.max(1, Math.floor(amounts[id] ?? 1));
      setBusy(id);
      try {
        setError(null);
        await onContribute(quest.questId, amount);
        await refresh();
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Contribution failed');
      } finally {
        setBusy(null);
      }
    },
    [amounts, onContribute, refresh],
  );

  const handleClaim = useCallback(
    async (quest: GuildQuestView) => {
      if (!onClaim) return;
      const id = String(quest.questId);
      setBusy(id);
      try {
        setError(null);
        await onClaim(quest.questId);
        await refresh();
      } catch (err) {
        setError(err instanceof Error ? err.message : 'Claim failed');
      } finally {
        setBusy(null);
      }
    },
    [onClaim, refresh],
  );

  return (
    <section
      aria-label="Guild Quest Board"
      style={{
        backgroundColor: '#1E293B',
        borderRadius: '12px',
        padding: '20px',
        border: '1px solid #334155',
        display: 'flex',
        flexDirection: 'column',
        gap: '16px',
      }}
    >
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <h2 style={{ margin: 0, color: '#F8FAFC', fontSize: '1.2rem' }}>Guild Quest Board</h2>
          <span style={{ color: '#94A3B8', fontSize: '0.8rem' }}>Alliance {String(allianceId)}</span>
        </div>
        {loadQuests && (
          <button
            type="button"
            onClick={() => void refresh()}
            style={{
              background: 'none',
              border: '1px solid #334155',
              borderRadius: '6px',
              color: '#38BDF8',
              padding: '6px 12px',
              cursor: 'pointer',
            }}
          >
            Refresh
          </button>
        )}
      </div>

      {error && (
        <div role="alert" style={{ color: '#FCA5A5', fontSize: '0.85rem' }}>
          {error}
        </div>
      )}

      <div role="tablist" aria-label="Filter guild quests" style={{ display: 'flex', gap: '8px' }}>
        {FILTERS.map(({ value, label }) => (
          <button
            key={value}
            type="button"
            role="tab"
            aria-selected={filter === value}
            onClick={() => setFilter(value)}
            style={{
              padding: '4px 10px',
              borderRadius: '9999px',
              border: '1px solid #334155',
              backgroundColor: filter === value ? '#334155' : 'transparent',
              color: filter === value ? '#F8FAFC' : '#94A3B8',
              cursor: 'pointer',
              fontSize: '0.8rem',
            }}
          >
            {label}
          </button>
        ))}
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(280px, 1fr))', gap: '12px' }}>
        {visible.length === 0 && (
          <p style={{ color: '#94A3B8', fontSize: '0.875rem' }}>No quests match this filter.</p>
        )}
        {visible.map((quest) => {
          const id = String(quest.questId);
          const status = effectiveStatus(quest, nowSecs);
          const colors = STATUS_COLORS[status];
          const frequency = quest.frequency ?? 'oneOff';
          const percent = Math.min(
            100,
            Math.round((quest.currentProgress / Math.max(1, quest.targetCount)) * 100),
          );
          const isBusy = busy === id;

          return (
            <article
              key={id}
              style={{
                backgroundColor: '#0F172A',
                borderRadius: '8px',
                padding: '14px',
                border: '1px solid #1E293B',
                display: 'flex',
                flexDirection: 'column',
                gap: '10px',
              }}
            >
              <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
                <strong style={{ color: '#F8FAFC', textTransform: 'capitalize' }}>
                  {quest.questType.replace(/_/g, ' ')}
                </strong>
                <span
                  style={{
                    padding: '2px 8px',
                    borderRadius: '9999px',
                    backgroundColor: colors.background,
                    color: colors.color,
                    fontSize: '0.7rem',
                    fontWeight: 600,
                    textTransform: 'uppercase',
                  }}
                >
                  {status}
                </span>
              </div>

              <div style={{ display: 'flex', flexWrap: 'wrap', gap: '6px' }}>
                <span style={{ color: '#94A3B8', fontSize: '0.75rem' }}>
                  {FREQUENCY_LABELS[frequency]}
                </span>
                <span style={{ color: '#A78BFA', fontSize: '0.75rem' }}>
                  Tier {quest.tier ?? 1}
                </span>
                {quest.chainId !== undefined && quest.chainId !== null && (
                  <span style={{ color: '#F59E0B', fontSize: '0.75rem' }}>
                    Chain #{String(quest.chainId)}
                  </span>
                )}
              </div>

              <div>
                <div style={{ display: 'flex', justifyContent: 'space-between', color: '#94A3B8', fontSize: '0.75rem' }}>
                  <span>Progress</span>
                  <span>
                    {quest.currentProgress.toLocaleString()} / {quest.targetCount.toLocaleString()}
                  </span>
                </div>
                <div
                  role="progressbar"
                  aria-valuenow={quest.currentProgress}
                  aria-valuemin={0}
                  aria-valuemax={quest.targetCount}
                  style={{ height: '6px', borderRadius: '4px', backgroundColor: '#1E293B', marginTop: '4px' }}
                >
                  <div
                    style={{
                      width: `${percent}%`,
                      height: '100%',
                      borderRadius: '4px',
                      backgroundColor: status === 'expired' ? '#EF4444' : '#38BDF8',
                    }}
                  />
                </div>
              </div>

              <div style={{ color: '#94A3B8', fontSize: '0.75rem' }}>
                <div>
                  {quest.contributors ?? 0} / {quest.minContributors ?? 1} contributors
                  {quest.assignees ? ` · ${quest.assignees} assigned` : ' · open to all'}
                </div>
                <div>
                  {formatAmount(quest.rewardEssence)} essence · {quest.rewardXp ?? 0} XP ·{' '}
                  {remainingTime(quest.expiresAt, nowSecs)}
                </div>
              </div>

              {status === 'active' && onContribute && (
                <div style={{ display: 'flex', gap: '8px' }}>
                  <input
                    aria-label={`Contribution amount for ${id}`}
                    type="number"
                    min={1}
                    value={amounts[id] ?? 1}
                    onChange={(event) =>
                      setAmounts((current) => ({
                        ...current,
                        [id]: Math.max(1, Number(event.target.value) || 1),
                      }))
                    }
                    style={{
                      width: '72px',
                      backgroundColor: '#1E293B',
                      border: '1px solid #334155',
                      borderRadius: '6px',
                      color: '#F8FAFC',
                      padding: '4px 8px',
                    }}
                  />
                  <button
                    type="button"
                    disabled={isBusy}
                    onClick={() => void handleContribute(quest)}
                    style={{
                      flex: 1,
                      backgroundColor: '#0284C7',
                      border: 'none',
                      borderRadius: '6px',
                      color: '#FFFFFF',
                      cursor: isBusy ? 'not-allowed' : 'pointer',
                      padding: '6px 10px',
                    }}
                  >
                    Contribute
                  </button>
                </div>
              )}

              {status === 'completed' && onClaim && (
                <button
                  type="button"
                  disabled={isBusy}
                  onClick={() => void handleClaim(quest)}
                  style={{
                    backgroundColor: '#047857',
                    border: 'none',
                    borderRadius: '6px',
                    color: '#FFFFFF',
                    cursor: isBusy ? 'not-allowed' : 'pointer',
                    padding: '6px 10px',
                  }}
                >
                  Claim reward
                </button>
              )}
            </article>
          );
        })}
      </div>
    </section>
  );
};
