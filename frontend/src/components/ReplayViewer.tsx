import React, { useCallback, useEffect, useState } from 'react';

/**
 * Playback UI for Issue #592 replays. Types mirror the Soroban `#[contracttype]`
 * structs in `src/combat_replay.rs` with camelCase field names.
 */
export type ReplaySpeed = 0.5 | 1 | 2;

export interface ReplayHeaderView {
  replayId: number;
  version: number;
  combatId: number;
  player1: string;
  player2: string;
  startedAt: number;
  endedAt: number;
  frameCount: number;
  finalTurn: number;
  winner?: string | null;
  status: string;
  integrityHash?: string;
}

export interface ReplayFrameViewData {
  index: number;
  kind: string;
  action: string;
  /** 0 = player1, 1 = player2, 4294967295 = no actor (setup frame). */
  actor: number;
  /** 0 = setup frame, 1-based for moves. */
  turnNumber: number;
  power: number;
  player1Hp: number;
  player2Hp: number;
  player1Energy: number;
  player2Energy: number;
  winner?: string | null;
  status: string;
  timestamp: number;
}

export interface ReplayBundleView {
  header: ReplayHeaderView;
  frames: ReplayFrameViewData[];
}

export interface ReplayViewerProps {
  replay?: ReplayBundleView | null;
  /** Token extracted from a `/replays?share=<token>` link. */
  shareToken?: string | null;
  /** Origin used when building share links. Defaults to `window.location.origin`. */
  baseUrl?: string;
  /** Parent hook: fetch the replay for `shareToken` and pass it back as `replay`. */
  onLoadShare?: (token: string) => void;
  onCopyShare?: (link: string) => void;
}

export const REPLAY_SPEEDS: ReplaySpeed[] = [0.5, 1, 2];

/** Clamp a seek target into `[0, frameCount)`. */
export function clampFrameIndex(frameCount: number, requested: number): number {
  if (frameCount <= 0) return 0;
  if (!Number.isFinite(requested)) return 0;
  return Math.max(0, Math.min(frameCount - 1, Math.floor(requested)));
}

/** Frame interval in milliseconds for the three supported speeds. */
export function intervalForSpeed(speed: ReplaySpeed): number {
  return Math.round(1000 / speed);
}

/** Build the canonical share link for a replay token. */
export function buildReplayShareUrl(token: string, baseUrl?: string): string {
  const origin = (baseUrl ?? (typeof window !== 'undefined' ? window.location.origin : '')).replace(
    /\/$/,
    '',
  );
  return `${origin}/replays?share=${encodeURIComponent(token)}`;
}

/** Read a share token from a location search/hash string. */
export function parseReplayShareToken(search: string): string | null {
  if (!search) return null;
  const query = search.replace(/^[?#]/, '');
  return new URLSearchParams(query).get('share');
}

const shortAddress = (address?: string | null): string =>
  address ? `${address.slice(0, 4)}…${address.slice(-4)}` : '—';

const StatBar: React.FC<{ label: string; value: number; color: string }> = ({ label, value, color }) => {
  const pct = Math.max(0, Math.min(100, value));
  return (
    <div style={{ marginBottom: '6px' }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', fontSize: '0.75rem', color: '#94A3B8' }}>
        <span>{label}</span>
        <span>{value}</span>
      </div>
      <div style={{ height: '6px', borderRadius: '4px', background: '#1E293B', overflow: 'hidden' }}>
        <div style={{ height: '100%', width: `${pct}%`, background: color, transition: 'width 150ms linear' }} />
      </div>
    </div>
  );
};

const controlButton = (active: boolean): React.CSSProperties => ({
  background: active ? '#3B82F6' : '#1E293B',
  color: active ? '#FFFFFF' : '#CBD5E1',
  border: '1px solid #334155',
  borderRadius: '6px',
  padding: '6px 12px',
  fontSize: '0.85rem',
  fontWeight: 600,
  cursor: 'pointer',
});

/** Deterministic replay player with play/pause, speed, frame seek and turn list. */
export const ReplayViewer: React.FC<ReplayViewerProps> = ({
  replay,
  shareToken,
  baseUrl,
  onLoadShare,
  onCopyShare,
}) => {
  const frames = replay?.frames ?? [];
  const [frameIndex, setFrameIndex] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState<ReplaySpeed>(1);
  const [copied, setCopied] = useState(false);

  const replayId = replay?.header.replayId ?? 0;

  useEffect(() => {
    setFrameIndex(0);
    setPlaying(false);
    setCopied(false);
  }, [replayId]);

  useEffect(() => {
    if (shareToken && onLoadShare) onLoadShare(shareToken);
  }, [shareToken, onLoadShare]);

  // One timeout per frame keeps timing exact at 0.5x / 1x / 2x without a
  // long-lived interval that would keep ticking after a seek.
  useEffect(() => {
    if (!playing || frames.length === 0) return;
    if (frameIndex >= frames.length - 1) {
      setPlaying(false);
      return;
    }
    const timer = window.setTimeout(() => {
      setFrameIndex((current) => clampFrameIndex(frames.length, current + 1));
    }, intervalForSpeed(speed));
    return () => window.clearTimeout(timer);
  }, [playing, frameIndex, frames.length, speed]);

  const seek = useCallback(
    (target: number) => {
      setPlaying(false);
      setFrameIndex(clampFrameIndex(frames.length, target));
    },
    [frames.length],
  );

  const activeFrame = frames[Math.min(frameIndex, Math.max(0, frames.length - 1))];
  const header = replay?.header;
  // The token comes from `share_combat_replay`; never invent one from the id.
  const linkToken = shareToken ?? '';
  const link = linkToken ? buildReplayShareUrl(linkToken, baseUrl) : '';

  const copyLink = useCallback(async () => {
    if (!link) return;
    try {
      if (typeof navigator !== 'undefined' && navigator.clipboard) {
        await navigator.clipboard.writeText(link);
      }
      onCopyShare?.(link);
      setCopied(true);
    } catch {
      setCopied(false);
    }
  }, [link, onCopyShare]);

  if (!replay || frames.length === 0) {
    return (
      <section
        data-testid="replay-viewer-empty"
        style={{ background: '#0F172A', color: '#E2E8F0', borderRadius: '10px', padding: '16px', border: '1px solid #334155' }}
      >
        <h3 style={{ margin: '0 0 8px', fontSize: '1.1rem' }}>Combat replay</h3>
        <p style={{ color: '#94A3B8', margin: 0 }}>
          {shareToken
            ? `Loading replay ${shareToken}…`
            : 'Open a replay link or select a recorded combat to watch it.'}
        </p>
      </section>
    );
  }

  return (
    <section
      data-testid="replay-viewer"
      style={{
        background: '#0F172A',
        color: '#E2E8F0',
        borderRadius: '10px',
        border: '1px solid #334155',
        padding: '16px',
        display: 'grid',
        gap: '14px',
      }}
    >
      <header style={{ display: 'flex', justifyContent: 'space-between', flexWrap: 'wrap', gap: '8px' }}>
        <div>
          <h3 style={{ margin: 0, fontSize: '1.1rem' }}>Combat replay #{header?.replayId}</h3>
          <small style={{ color: '#94A3B8' }}>
            Combat #{header?.combatId} · {header?.frameCount} frames · {header?.status}
            {header?.winner ? ` · winner ${shortAddress(header.winner)}` : ''}
          </small>
        </div>
        {link ? (
          <div style={{ display: 'flex', alignItems: 'center', gap: '8px' }}>
            <input
              aria-label="Replay share link"
              readOnly
              value={link}
              onFocus={(event) => event.currentTarget.select()}
              style={{
                background: '#1E293B',
                color: '#94A3B8',
                border: '1px solid #334155',
                borderRadius: '6px',
                padding: '6px 8px',
                minWidth: '240px',
                fontSize: '0.8rem',
              }}
            />
            <button type="button" style={controlButton(false)} onClick={() => void copyLink()}>
              {copied ? 'Copied' : 'Copy link'}
            </button>
          </div>
        ) : (
          <small style={{ color: '#64748B' }}>Publish a share token to create a link.</small>
        )}
      </header>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fit, minmax(200px, 1fr))', gap: '12px' }}>
        <div style={{ background: '#1E293B', borderRadius: '8px', padding: '10px' }}>
          <strong style={{ display: 'block', marginBottom: '8px' }}>{shortAddress(header?.player1)}</strong>
          <StatBar label="Hull" value={activeFrame?.player1Hp ?? 0} color="#38BDF8" />
          <StatBar label="Energy" value={activeFrame?.player1Energy ?? 0} color="#A78BFA" />
        </div>
        <div style={{ background: '#1E293B', borderRadius: '8px', padding: '10px' }}>
          <strong style={{ display: 'block', marginBottom: '8px' }}>{shortAddress(header?.player2)}</strong>
          <StatBar label="Hull" value={activeFrame?.player2Hp ?? 0} color="#F472B6" />
          <StatBar label="Energy" value={activeFrame?.player2Energy ?? 0} color="#FBBF24" />
        </div>
      </div>

      <div style={{ color: '#CBD5E1', fontSize: '0.9rem' }} data-testid="replay-current-frame">
        Turn {activeFrame?.turnNumber ?? 0} · {activeFrame?.kind ?? 'start'} / {activeFrame?.action ?? 'none'}
        {activeFrame && activeFrame.power > 0 ? ` · power ${activeFrame.power}` : ''}
        {activeFrame?.status ? ` · ${activeFrame.status}` : ''}
      </div>

      <div style={{ display: 'flex', flexWrap: 'wrap', alignItems: 'center', gap: '8px' }}>
        <button
          type="button"
          aria-label={playing ? 'Pause replay' : 'Play replay'}
          data-testid="replay-play-pause"
          style={controlButton(playing)}
          onClick={() => {
            if (!playing && frameIndex >= frames.length - 1) setFrameIndex(0);
            setPlaying((value) => !value);
          }}
        >
          {playing ? 'Pause' : 'Play'}
        </button>
        <button type="button" aria-label="Previous frame" style={controlButton(false)} onClick={() => seek(frameIndex - 1)}>
          ◀ Frame
        </button>
        <button type="button" aria-label="Next frame" style={controlButton(false)} onClick={() => seek(frameIndex + 1)}>
          Frame ▶
        </button>

        <div style={{ display: 'inline-flex', gap: '4px', marginLeft: '8px' }} role="group" aria-label="Playback speed">
          {REPLAY_SPEEDS.map((value) => (
            <button
              key={value}
              type="button"
              data-testid={`replay-speed-${value}`}
              aria-pressed={speed === value}
              style={controlButton(speed === value)}
              onClick={() => setSpeed(value)}
            >
              {value}x
            </button>
          ))}
        </div>

        <span style={{ color: '#94A3B8', fontSize: '0.8rem', marginLeft: 'auto' }}>
          {intervalForSpeed(speed)} ms / frame
        </span>
      </div>

      <label style={{ display: 'grid', gap: '4px', color: '#94A3B8', fontSize: '0.8rem' }}>
        Frame seek
        <input
          type="range"
          aria-label="Seek replay"
          data-testid="replay-seek"
          min={0}
          max={Math.max(0, frames.length - 1)}
          step={1}
          value={frameIndex}
          onChange={(event) => seek(Number(event.target.value))}
          style={{ width: '100%' }}
        />
      </label>

      <div>
        <strong style={{ color: '#94A3B8', fontSize: '0.8rem' }}>Turns</strong>
        <div
          data-testid="replay-turn-list"
          style={{ display: 'flex', gap: '6px', overflowX: 'auto', padding: '8px 0' }}
        >
          {frames.map((frame) => (
            <button
              key={frame.index}
              type="button"
              data-testid={`replay-turn-${frame.turnNumber}`}
              aria-label={`Seek to turn ${frame.turnNumber}`}
              aria-current={frame.index === frameIndex ? 'step' : undefined}
              style={controlButton(frame.index === frameIndex)}
              onClick={() => seek(frame.index)}
              title={`${frame.kind} / ${frame.action} · power ${frame.power}`}
            >
              {frame.turnNumber === 0 ? 'Start' : `T${frame.turnNumber}`}
            </button>
          ))}
        </div>
      </div>
    </section>
  );
};

export default ReplayViewer;
