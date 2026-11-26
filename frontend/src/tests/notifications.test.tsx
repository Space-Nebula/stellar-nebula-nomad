import React from 'react';
import { fireEvent, render, screen } from '@testing-library/react';
import { NotificationCenter } from '../views/NotificationCenter';
import { CHANNEL_IN_APP, CHANNEL_PUSH, GameNotification, isDelivered, NotificationPreferences, sortInbox } from '../utils/notifications';

const prefs: NotificationPreferences = { minPriority: 'Normal', channels: CHANNEL_IN_APP, muted: ['craft_ok'] };

describe('notification rule mirrors', () => {
  test('filter mirrors the contract: priority floor, mute, urgent bypass', () => {
    expect(isDelivered(prefs, 'rare_find', 'Low')).toBe(false);
    expect(isDelivered(prefs, 'rare_find', 'Normal')).toBe(true);
    expect(isDelivered(prefs, 'craft_ok', 'High')).toBe(false);
    expect(isDelivered(prefs, 'craft_ok', 'Urgent')).toBe(true);
  });

  test('inbox sorts by priority then recency', () => {
    const inbox: GameNotification[] = [
      { message: 'a', timestamp: 3, priority: 'Low', count: 1 },
      { message: 'b', timestamp: 1, priority: 'Urgent', count: 1 },
      { message: 'c', timestamp: 2, priority: 'Low', count: 1 },
    ];
    expect(sortInbox(inbox).map((n) => n.message)).toEqual(['b', 'a', 'c']);
  });
});

describe('NotificationCenter', () => {
  const inbox: GameNotification[] = [
    { message: 'low_res', timestamp: 10, priority: 'High', count: 3 },
    { message: 'rare_find', timestamp: 20, priority: 'Normal', count: 1 },
  ];

  test('shows aggregated counts in priority order and mutes', () => {
    const onChange = jest.fn();
    render(<NotificationCenter inbox={inbox} preferences={prefs} onPreferencesChange={onChange} onClear={jest.fn()} />);
    const items = screen.getAllByRole('listitem');
    expect(items[0].textContent).toContain('low_res');
    expect(screen.getByTestId('notif-low_res').textContent).toContain('×3');
    fireEvent.click(screen.getAllByText('Mute')[0]);
    expect(onChange).toHaveBeenCalledWith({ ...prefs, muted: ['craft_ok', 'low_res'] });
  });

  test('edits channels and minimum priority', () => {
    const onChange = jest.fn();
    render(<NotificationCenter inbox={[]} preferences={prefs} onPreferencesChange={onChange} onClear={jest.fn()} />);
    expect(screen.getByText('No notifications.')).toBeDefined();
    fireEvent.click(screen.getByLabelText('Push'));
    expect(onChange).toHaveBeenLastCalledWith({ ...prefs, channels: CHANNEL_IN_APP | CHANNEL_PUSH });
    fireEvent.change(screen.getByLabelText('Minimum priority'), { target: { value: 'High' } });
    expect(onChange).toHaveBeenLastCalledWith({ ...prefs, minPriority: 'High' });
  });
});
