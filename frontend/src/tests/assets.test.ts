import { responsiveImage } from '../utils/assets';

describe('responsiveImage', () => {
  test('adds lazy loading and responsive metadata', () => {
    const result = responsiveImage('/assets/ship.webp');

    expect(result.loading).toBe('lazy');
    expect(result.decoding).toBe('async');
    expect(result.src).toBe('/assets/ship.webp');
    expect(result.srcSet).toBeUndefined();
    expect(result.sizes).toContain('vw');
  });

  test('builds CDN URLs and srcset entries', () => {
    const result = responsiveImage('/assets/ship.webp', {
      cdnBaseUrl: 'https://cdn.example.test/',
      widths: [320, 640],
    });

    expect(result.src).toContain('https://cdn.example.test/assets/ship.webp');
    expect(result.srcSet).toContain('w=320&q=75&f=auto 320w');
    expect(result.srcSet).toContain('w=640&q=75&f=auto 640w');
  });
});
