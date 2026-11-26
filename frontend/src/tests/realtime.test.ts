import { toRealtimeNotification, resolveRealtimeUrl } from '../services/realtime';
import type { WebSocketEvent } from '../services/websocket';

describe('realtime helpers', () => {
  test('resolves an explicit websocket URL', () => {
    expect(resolveRealtimeUrl('wss://example.test/ws')).toBe(
      'wss://example.test/ws',
    );
  });

  test('normalizes notification events', () => {
    const event: WebSocketEvent = {
      id: 'notice-1',
      type: 'notification',
      channel: 'notifications',
      payload: {
        id: 'notice-1',
        message: 'Harvest complete',
        severity: 'success',
        timestamp: 123,
      },
      sequence: 4,
      timestamp: 456,
    };

    expect(toRealtimeNotification(event)).toMatchObject({
      message: 'Harvest complete',
      severity: 'success',
      timestamp: 123,
    });
  });

  test('ignores non-notification events', () => {
    expect(
      toRealtimeNotification({
        id: 'price-1',
        type: 'price_update',
        channel: 'market_prices',
        payload: {},
        sequence: 1,
        timestamp: 1,
      }),
    ).toBeNull();
  });
});
