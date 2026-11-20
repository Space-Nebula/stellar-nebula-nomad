import React from 'react';
import { colors, visuallyHidden } from '../../a11y';
import { useI18n } from '../../i18n';

export interface LeaderboardRow {
  rank: number;
  playerId: string;
  score: number;
  guild?: string;
}

export interface LeaderboardTableProps {
  entries: LeaderboardRow[];
  currentPlayerId?: string;
}

export const LeaderboardTable: React.FC<LeaderboardTableProps> = ({ entries, currentPlayerId }) => {
  const { t, formatNumber } = useI18n();
  const cell: React.CSSProperties = { padding: '10px 12px' };
  return (
    <div style={{ overflowX: 'auto', width: '100%' }}>
      <table
        style={{
          width: '100%',
          borderCollapse: 'collapse',
          fontSize: '0.875rem',
          textAlign: 'start',
        }}
      >
        <caption style={visuallyHidden}>{t('leaderboard.label')}</caption>
        <thead>
          <tr style={{ borderBottom: `1px solid ${colors.border}`, color: colors.textMuted }}>
            <th scope="col" style={{ ...cell, textAlign: 'start' }}>{t('leaderboard.rank')}</th>
            <th scope="col" style={{ ...cell, textAlign: 'start' }}>{t('leaderboard.explorer')}</th>
            <th scope="col" style={{ ...cell, textAlign: 'start' }}>{t('leaderboard.guild')}</th>
            <th scope="col" style={{ ...cell, textAlign: 'end' }}>{t('leaderboard.score')}</th>
          </tr>
        </thead>
        <tbody>
          {entries.map((entry) => {
            const isCurrent =
              !!currentPlayerId && entry.playerId.toLowerCase() === currentPlayerId.toLowerCase();
            return (
              <tr
                key={entry.rank}
                aria-current={isCurrent ? 'true' : undefined}
                style={{
                  backgroundColor: isCurrent ? 'rgba(56, 189, 248, 0.1)' : 'transparent',
                  borderBottom: `1px solid ${colors.bgRaised}`,
                  // Not colour alone: the current row also gets a thick edge and "(you)".
                  boxShadow: isCurrent ? `inset 4px 0 0 ${colors.accent}` : undefined,
                }}
              >
                <th
                  scope="row"
                  style={{
                    ...cell,
                    textAlign: 'start',
                    fontWeight: 700,
                    color: entry.rank <= 3 ? colors.warning : colors.textMuted,
                  }}
                >
                  #{formatNumber(entry.rank)}
                </th>
                <td style={{ ...cell, color: colors.textPrimary, fontFamily: 'monospace' }}>
                  <span dir="ltr" title={entry.playerId}>
                    {entry.playerId.slice(0, 6)}...{entry.playerId.slice(-4)}
                  </span>
                  {isCurrent && <span style={{ marginInlineStart: 6 }}>{t('common.you')}</span>}
                </td>
                <td style={{ ...cell, color: colors.accent }}>{entry.guild || '-'}</td>
                <td style={{ ...cell, textAlign: 'end', fontWeight: 600, color: colors.success }}>
                  {formatNumber(entry.score)}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
};
