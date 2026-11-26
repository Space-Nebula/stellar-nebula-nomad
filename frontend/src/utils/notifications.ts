/**
 * Client-side mirror of the on-chain notification rules (#710). Keep in sync
 * with src/notifications/push_service.rs.
 */

export type NotificationPriority = 'Low' | 'Normal' | 'High' | 'Urgent';

export const PRIORITIES: NotificationPriority[] = ['Low', 'Normal', 'High', 'Urgent'];

export const CHANNEL_IN_APP = 1;
export const CHANNEL_PUSH = 2;
export const CHANNEL_EMAIL = 4;
export const CHANNELS: { bit: number; label: string }[] = [
  { bit: CHANNEL_IN_APP, label: 'In-app' },
  { bit: CHANNEL_PUSH, label: 'Push' },
  { bit: CHANNEL_EMAIL, label: 'Email' },
];
export const MAX_MUTED = 16;

export interface GameNotification {
  message: string;
  /** Ledger time of the latest occurrence. */
  timestamp: number;
  priority: NotificationPriority;
  /** Occurrences folded into this entry. */
  count: number;
}

export interface NotificationPreferences {
  minPriority: NotificationPriority;
  channels: number;
  muted: string[];
}

export function priorityRank(p: NotificationPriority): number {
  return PRIORITIES.indexOf(p);
}

/** Mirrors the `notify` filter: Urgent always passes. */
export function isDelivered(prefs: NotificationPreferences, message: string, priority: NotificationPriority): boolean {
  if (priority === 'Urgent') return true;
  return priorityRank(priority) >= priorityRank(prefs.minPriority) && !prefs.muted.includes(message);
}

/** Inbox display order: highest priority first, then newest. */
export function sortInbox(inbox: GameNotification[]): GameNotification[] {
  return [...inbox].sort((a, b) => priorityRank(b.priority) - priorityRank(a.priority) || b.timestamp - a.timestamp);
}

export function toggleChannel(channels: number, bit: number): number {
  return channels ^ bit;
}
