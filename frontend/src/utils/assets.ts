export interface ResponsiveImageSources {
  src: string;
  srcSet?: string;
  sizes?: string;
  loading: 'lazy';
  decoding: 'async';
}

export interface AssetDeliveryConfig {
  cdnBaseUrl?: string;
  widths?: number[];
  quality?: number;
  sizes?: string;
}

const DEFAULT_WIDTHS = [320, 640, 960, 1280];
const DEFAULT_QUALITY = 75;

function normalizeCdnBase(cdnBaseUrl?: string): string {
  if (!cdnBaseUrl) {
    return '';
  }
  return cdnBaseUrl.endsWith('/') ? cdnBaseUrl.slice(0, -1) : cdnBaseUrl;
}

function normalizePath(path: string): string {
  return path.startsWith('/') ? path : `/${path}`;
}

function withCdnParams(
  cdnBaseUrl: string | undefined,
  path: string,
  width: number,
  quality: number,
): string {
  const base = normalizeCdnBase(cdnBaseUrl);
  if (!base) {
    return path;
  }

  const separator = path.includes('?') ? '&' : '?';
  return `${base}${normalizePath(path)}${separator}w=${width}&q=${quality}&f=auto`;
}

export function responsiveImage(
  path: string,
  config: AssetDeliveryConfig = {},
): ResponsiveImageSources {
  const widths = config.widths ?? DEFAULT_WIDTHS;
  const quality = config.quality ?? DEFAULT_QUALITY;
  const cdnBaseUrl = config.cdnBaseUrl ?? readCdnBaseUrl();

  return {
    src: withCdnParams(cdnBaseUrl, path, widths[Math.floor(widths.length / 2)], quality),
    srcSet: cdnBaseUrl
      ? widths
          .map(
            (width) =>
              `${withCdnParams(cdnBaseUrl, path, width, quality)} ${width}w`,
          )
          .join(', ')
      : undefined,
    sizes: config.sizes ?? '(max-width: 768px) 100vw, 33vw',
    loading: 'lazy',
    decoding: 'async',
  };
}

function readCdnBaseUrl(): string | undefined {
  if (typeof window === 'undefined') {
    return undefined;
  }
  const runtime = window as unknown as {
    __NEBULA_CDN_URL__?: string;
    env?: Record<string, string | undefined>;
  };
  return runtime.__NEBULA_CDN_URL__ ?? runtime.env?.NEBULA_CDN_URL;
}
