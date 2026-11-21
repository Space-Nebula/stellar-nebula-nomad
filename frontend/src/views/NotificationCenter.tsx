import React from 'react';
import {
  CHANNELS,
  GameNotification,
  MAX_MUTED,
  NotificationPreferences,
  NotificationPriority,
  PRIORITIES,
  sortInbox,
  toggleChannel,
} from '../utils/notifications';

export interface NotificationCenterProps {
  /** Player inbox (`get_inbox`). */
  inbox: GameNotification[];
  preferences: NotificationPreferences;
  onPreferencesChange: (prefs: NotificationPreferences) => void;
  onClear: () => void;
}

const PRIORITY_COLOR: Record<NotificationPriority, string> = {
  Low: '#94A3B8',
  Normal: '#60A5FA',
  High: '#FBBF24',
  Urgent: '#F87171',
};

/** Inbox with priorities and aggregated repeats, plus delivery preferences (#710). */
export const NotificationCenter: React.FC<NotificationCenterProps> = ({ inbox, preferences, onPreferencesChange, onClear }) => {
  const toggleMute = (message: string) => {
    const muted = preferences.muted.includes(message)
      ? preferences.muted.filter((m) => m !== message)
      : [...preferences.muted, message];
    onPreferencesChange({ ...preferences, muted });
  };
  const muteFull = preferences.muted.length >= MAX_MUTED;

  return (
    <section aria-label="Notifications" style={{ padding: 16, borderRadius: 8, background: '#0F172A', color: '#F8FAFC' }}>
      <header style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <h2 style={{ margin: 0, fontSize: '1.1rem' }}>Notifications</h2>
        <button type="button" onClick={onClear} disabled={inbox.length === 0}>
          Clear all
        </button>
      </header>

      {inbox.length === 0 ? (
        <p>No notifications.</p>
      ) : (
        <ul aria-live="polite" style={{ listStyle: 'none', padding: 0 }}>
          {sortInbox(inbox).map((n) => {
            const muted = preferences.muted.includes(n.message);
            return (
              <li key={n.message} data-testid={`notif-${n.message}`} style={{ padding: 8, borderInlineStart: `4px solid ${PRIORITY_COLOR[n.priority]}`, marginBottom: 6 }}>
                <strong>{n.priority}</strong> · {n.message}
                {n.count > 1 && <span> (×{n.count})</span>}
                <button
                  type="button"
                  aria-pressed={muted}
                  disabled={!muted && muteFull}
                  onClick={() => toggleMute(n.message)}
                  style={{ marginInlineStart: 8 }}
                >
                  {muted ? 'Unmute' : 'Mute'}
                </button>
              </li>
            );
          })}
        </ul>
      )}

      <fieldset>
        <legend>Delivery</legend>
        <label>
          Minimum priority{' '}
          <select
            value={preferences.minPriority}
            onChange={(e) => onPreferencesChange({ ...preferences, minPriority: e.target.value as NotificationPriority })}
          >
            {PRIORITIES.map((p) => (
              <option key={p} value={p}>
                {p}
              </option>
            ))}
          </select>
        </label>
        {CHANNELS.map((c) => (
          <label key={c.bit} style={{ marginInlineStart: 12 }}>
            <input
              type="checkbox"
              checked={(preferences.channels & c.bit) !== 0}
              onChange={() => onPreferencesChange({ ...preferences, channels: toggleChannel(preferences.channels, c.bit) })}
            />{' '}
            {c.label}
          </label>
        ))}
        <p style={{ color: '#94A3B8', fontSize: '0.8rem' }}>Urgent notifications always reach your in-app inbox.</p>
      </fieldset>
    </section>
  );
};
