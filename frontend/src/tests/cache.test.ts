import { ApplicationCache } from '../services/cache';

describe('ApplicationCache', () => {
  beforeEach(() => {
    localStorage.clear();
  });

  test('stores and reads values with expiry metadata', () => {
    const cache = new ApplicationCache({ namespace: 'test', ttlMs: 60_000 });
    cache.set('key', { value: 1 });

    expect(cache.get('key')).toEqual({ value: 1 });
    expect(cache.get('missing')).toBeUndefined();
    cache.invalidate('key');
    expect(cache.get('key')).toBeUndefined();
  });

  test('loads missing values once', async () => {
    const cache = new ApplicationCache({ namespace: 'load', ttlMs: 60_000 });
    const loader = jest.fn(async () => ({ loaded: true }));

    await expect(cache.getOrLoad('key', loader)).resolves.toEqual({
      loaded: true,
    });
    await cache.getOrLoad('key', loader);
    expect(loader).toHaveBeenCalledTimes(1);
  });

  test('clears the whole namespace', () => {
    const cache = new ApplicationCache({
      namespace: 'clearable',
      ttlMs: 60_000,
    });
    cache.set('a', 1);
    cache.set('b', 2);
    cache.clear();

    expect(cache.get('a')).toBeUndefined();
    expect(cache.get('b')).toBeUndefined();
  });
});
