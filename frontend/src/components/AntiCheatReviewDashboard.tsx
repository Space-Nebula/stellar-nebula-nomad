import React, { useMemo, useState } from 'react';

/**
 * Admin review dashboard for Issue #593 anti-cheat detections.
 * Mirrors `anti_cheat::DetectionRecord` / `anti_cheat::AppealRecord`; all
 * timestamps are ledger seconds.
 */
export type AntiCheatSeverity = 'low' | 'medium' | 'high' | 'critical';
export type AntiCheatDetectionType = 'invalid' | 'impossible' | 'timing' | 'pattern' | 'burst';
export type AntiCheatPenalty = 'none' | 'warning' | 'ban_temp' | 'ban_perm';
export type AntiCheatReviewDecision = 'pending' | 'cleared' | 'upheld' | 'reversed';
export type AntiCheatAppealStatus = 'pending' | 'approved' | 'rejected';

export interface AntiCheatDetectionView {
  detectionId: number;
  player: string;
  detectionType: AntiCheatDetectionType;
  severity: AntiCheatSeverity;
  scoreDelta: number;
  scoreAfter: number;
  combatId?: number | null;
  action: string;
  detail: string;
  /** Ledger seconds. */
  timestamp: number;
  autoPenalty: AntiCheatPenalty;
  reviewed: boolean;
  reviewDecision?: AntiCheatReviewDecision | null;
}

export interface AntiCheatAppealView {
  appealId: number;
  detectionId: number;
  player: string;
  reason: string;
  createdAt: number;
  deadline: number;
  status: AntiCheatAppealStatus;
  reviewer?: string | null;
  resolvedAt: number;
}

export interface AntiCheatConfigView {
  reviewThreshold: number;
  warningThreshold: number;
  tempBanThreshold: number;
  permanentBanThreshold: number;
  minActionIntervalSecs: number;
  actionWindowSecs: number;
  maxActionsPerWindow: number;
  detectionCooldownSecs: number;
  tempBanDurationSecs: number;
  appealWindowSecs: number;
  maxDetectionsPerPlayer: number;
  maxReviewQueue: number;
}

export interface AntiCheatReviewDashboardProps {
  detections: AntiCheatDetectionView[];
  appeals?: AntiCheatAppealView[];
  config?: AntiCheatConfigView;
  onResolveReview?: (
    detectionId: number,
    decision: AntiCheatReviewDecision,
    reason: string,
  ) => void | Promise<void>;
  onResolveAppeal?: (
    appealId: number,
    approve: boolean,
    reason: string,
  ) => void | Promise<void>;
}

type Tab = 'queue' | 'appeals' | 'config';

const SEVERITY_COLOR: Record<AntiCheatSeverity, string> = {
  low: '#38BDF8',
  medium: '#FBBF24',
  high: '#FB923C',
  critical: '#F87171',
};

const short = (addr?: string | null) =>
  addr ? `${addr.slice(0, 6)}…${addr.slice(-4)}` : 'unknown';

const when = (seconds: number) =>
  seconds > 0 ? new Date(seconds * 1000).toISOString().replace('T', ' ').slice(0, 19) : '—';

const cardStyle: React.CSSProperties = {
  background: '#0F172A',
  color: '#E2E8F0',
  border: '1px solid #334155',
  borderRadius: '8px',
  padding: '12px',
};

const buttonStyle: React.CSSProperties = {
  background: '#1E293B',
  color: '#E2E8F0',
  border: '1px solid #475569',
  borderRadius: '6px',
  padding: '6px 10px',
  cursor: 'pointer',
};

/** Queue, appeal and threshold views for anti-cheat administrators. */
export const AntiCheatReviewDashboard: React.FC<AntiCheatReviewDashboardProps> = ({
  detections,
  appeals = [],
  config,
  onResolveReview,
  onResolveAppeal,
}) => {
  const [tab, setTab] = useState<Tab>('queue');
  const [severityFilter, setSeverityFilter] = useState<AntiCheatSeverity | 'all'>('all');
  const [selectedId, setSelectedId] = useState<number | null>(null);
  const [reason, setReason] = useState('');

  const pending = useMemo(() => detections.filter((d) => !d.reviewed), [detections]);
  const filtered = useMemo(
    () =>
      pending.filter(
        (d) => severityFilter === 'all' || d.severity === severityFilter,
      ),
    [pending, severityFilter],
  );
  const selected = filtered.find((d) => d.detectionId === selectedId) ?? null;
  const pendingAppeals = appeals.filter((a) => a.status === 'pending');

  const decide = (decision: AntiCheatReviewDecision) => {
    if (selected) {
      void onResolveReview?.(selected.detectionId, decision, reason.trim() || decision);
      setReason('');
      setSelectedId(null);
    }
  };

  return (
    <section data-testid="anti-cheat-dashboard" style={{ display: 'grid', gap: '16px' }}>
      <header style={{ display: 'flex', gap: '12px', alignItems: 'center', flexWrap: 'wrap' }}>
        <h2 style={{ margin: 0, color: '#E2E8F0' }}>Anti-Cheat Review</h2>
        <span style={{ color: '#94A3B8' }}>{pending.length} queued</span>
        <span style={{ color: '#94A3B8' }}>{pendingAppeals.length} appeals pending</span>
        <nav style={{ display: 'flex', gap: '6px', marginLeft: 'auto' }}>
          {(['queue', 'appeals', 'config'] as Tab[]).map((t) => (
            <button
              key={t}
              type="button"
              onClick={() => setTab(t)}
              style={{
                ...buttonStyle,
                borderColor: tab === t ? '#38BDF8' : '#475569',
              }}
            >
              {t === 'queue' ? 'Review queue' : t === 'appeals' ? 'Appeals' : 'Thresholds'}
            </button>
          ))}
        </nav>
      </header>

      {tab === 'queue' && (
        <div style={{ display: 'grid', gridTemplateColumns: 'minmax(280px, 1fr) minmax(300px, 1fr)', gap: '16px' }}>
          <div style={{ display: 'grid', gap: '8px' }}>
            <label style={{ color: '#94A3B8', fontSize: '13px' }}>
              Severity filter{' '}
              <select
                value={severityFilter}
                onChange={(e) => setSeverityFilter(e.target.value as AntiCheatSeverity | 'all')}
                style={{ ...buttonStyle, marginLeft: '6px' }}
              >
                <option value="all">all</option>
                <option value="low">low</option>
                <option value="medium">medium</option>
                <option value="high">high</option>
                <option value="critical">critical</option>
              </select>
            </label>

            {filtered.length === 0 && (
              <p style={{ color: '#94A3B8' }}>No detections match this filter.</p>
            )}
            {filtered.map((d) => (
              <button
                key={d.detectionId}
                type="button"
                data-testid={`detection-${d.detectionId}`}
                onClick={() => setSelectedId(d.detectionId)}
                style={{
                  ...cardStyle,
                  textAlign: 'left',
                  cursor: 'pointer',
                  borderColor:
                    selectedId === d.detectionId ? '#38BDF8' : SEVERITY_COLOR[d.severity],
                }}
              >
                <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                  <strong>#{d.detectionId} {d.detectionType}</strong>
                  <span style={{ color: SEVERITY_COLOR[d.severity] }}>{d.severity}</span>
                </div>
                <small style={{ color: '#94A3B8' }}>
                  {short(d.player)} · score {d.scoreAfter} (+{d.scoreDelta}) · {d.detail}
                  {d.combatId != null ? ` · combat #${d.combatId}` : ''}
                </small>
                <div style={{ color: '#CBD5E1', fontSize: '12px' }}>
                  {when(d.timestamp)} · penalty {d.autoPenalty}
                </div>
              </button>
            ))}
          </div>

          <aside style={cardStyle}>
            <h3 style={{ marginTop: 0 }}>Detection detail</h3>
            {!selected ? (
              <p style={{ color: '#94A3B8' }}>Select a detection to review it.</p>
            ) : (
              <>
                <dl style={{ display: 'grid', gridTemplateColumns: '120px 1fr', gap: '6px' }}>
                  <dt style={{ color: '#94A3B8' }}>ID</dt>
                  <dd>{selected.detectionId}</dd>
                  <dt style={{ color: '#94A3B8' }}>Player</dt>
                  <dd>{selected.player}</dd>
                  <dt style={{ color: '#94A3B8' }}>Rule</dt>
                  <dd>{selected.detectionType} / {selected.detail}</dd>
                  <dt style={{ color: '#94A3B8' }}>Severity</dt>
                  <dd style={{ color: SEVERITY_COLOR[selected.severity] }}>{selected.severity}</dd>
                  <dt style={{ color: '#94A3B8' }}>Score</dt>
                  <dd>{selected.scoreAfter} (+{selected.scoreDelta})</dd>
                  <dt style={{ color: '#94A3B8' }}>Action</dt>
                  <dd>{selected.action}</dd>
                  <dt style={{ color: '#94A3B8' }}>Auto penalty</dt>
                  <dd>{selected.autoPenalty}</dd>
                  <dt style={{ color: '#94A3B8' }}>Time</dt>
                  <dd>{when(selected.timestamp)}</dd>
                </dl>
                <textarea
                  aria-label="Review reason"
                  placeholder="Review note (stored in the review event)"
                  value={reason}
                  onChange={(e) => setReason(e.target.value)}
                  rows={3}
                  style={{ width: '100%', margin: '8px 0', background: '#020617', color: '#E2E8F0', border: '1px solid #334155', borderRadius: '6px' }}
                />
                <div style={{ display: 'flex', gap: '8px', flexWrap: 'wrap' }}>
                  <button type="button" style={buttonStyle} onClick={() => decide('upheld')}>Uphold</button>
                  <button type="button" style={buttonStyle} onClick={() => decide('cleared')}>Clear</button>
                  <button type="button" style={buttonStyle} onClick={() => decide('reversed')}>Reverse</button>
                </div>
              </>
            )}
          </aside>
        </div>
      )}

      {tab === 'appeals' && (
        <div style={{ display: 'grid', gap: '8px' }}>
          {appeals.length === 0 && <p style={{ color: '#94A3B8' }}>No appeals filed.</p>}
          {appeals.map((a) => (
            <div key={a.appealId} data-testid={`appeal-${a.appealId}`} style={cardStyle}>
              <div style={{ display: 'flex', justifyContent: 'space-between' }}>
                <strong>Appeal #{a.appealId} · detection #{a.detectionId}</strong>
                <span style={{ color: a.status === 'pending' ? '#FBBF24' : '#94A3B8' }}>{a.status}</span>
              </div>
              <small style={{ color: '#94A3B8' }}>
                {short(a.player)} · filed {when(a.createdAt)} · deadline {when(a.deadline)}
              </small>
              <p style={{ margin: '6px 0', color: '#CBD5E1' }}>{a.reason}</p>
              {a.status === 'pending' && (
                <div style={{ display: 'flex', gap: '8px' }}>
                  <button
                    type="button"
                    style={buttonStyle}
                    onClick={() => void onResolveAppeal?.(a.appealId, true, reason.trim() || 'appeal approved')}
                  >
                    Approve
                  </button>
                  <button
                    type="button"
                    style={buttonStyle}
                    onClick={() => void onResolveAppeal?.(a.appealId, false, reason.trim() || 'appeal rejected')}
                  >
                    Reject
                  </button>
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {tab === 'config' && (
        <div style={cardStyle}>
          <h3 style={{ marginTop: 0 }}>Scoring thresholds</h3>
          {config ? (
            <dl style={{ display: 'grid', gridTemplateColumns: '220px 1fr', gap: '6px' }}>
              {Object.entries(config).map(([key, value]) => (
                <React.Fragment key={key}>
                  <dt style={{ color: '#94A3B8' }}>{key}</dt>
                  <dd>{String(value)}</dd>
                </React.Fragment>
              ))}
            </dl>
          ) : (
            <p style={{ color: '#94A3B8' }}>No configuration loaded; contract defaults apply.</p>
          )}
        </div>
      )}
    </section>
  );
};

export default AntiCheatReviewDashboard;
