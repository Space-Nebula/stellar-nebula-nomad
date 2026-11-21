/**
 * Locale-aware formatting built on `Intl` (Issue #697).
 *
 * Each formatter takes the locale explicitly so it can be used outside React;
 * `useI18n()` binds them to the active language.
 */

export const formatNumber = (
  locale: string,
  value: number,
  options?: Intl.NumberFormatOptions
): string => new Intl.NumberFormat(locale, options).format(value);

export const formatPercent = (
  locale: string,
  ratio: number,
  options?: Intl.NumberFormatOptions
): string =>
  new Intl.NumberFormat(locale, { style: 'percent', maximumFractionDigits: 1, ...options }).format(
    ratio
  );

/**
 * Format an amount in an ISO 4217 currency. Crypto assets such as `XLM` are
 * not ISO codes, so they fall back to a plain number followed by the symbol.
 */
export const formatCurrency = (locale: string, value: number, currency: string): string => {
  if (isIsoCurrency(currency)) {
    return new Intl.NumberFormat(locale, { style: 'currency', currency }).format(value);
  }
  // Stellar amounts have up to 7 decimal places.
  return `${formatNumber(locale, value, { maximumFractionDigits: 7 })} ${currency}`;
};

let isoCurrencies: Set<string> | undefined;

/**
 * `Intl.NumberFormat` accepts any well-formed 3-letter code (it would render
 * "XLM 12.50"), so check the runtime's list of real ISO 4217 currencies.
 */
const isIsoCurrency = (code: string): boolean => {
  const intl = Intl as typeof Intl & { supportedValuesOf?: (key: 'currency') => string[] };
  if (!intl.supportedValuesOf) return /^[A-Z]{3}$/.test(code) && code !== 'XLM';
  isoCurrencies ??= new Set(intl.supportedValuesOf('currency'));
  return isoCurrencies.has(code.toUpperCase());
};

export const formatDate = (
  locale: string,
  value: Date | number,
  options: Intl.DateTimeFormatOptions = { dateStyle: 'medium', timeStyle: 'short' }
): string => new Intl.DateTimeFormat(locale, options).format(value);

const RELATIVE_UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ['year', 365 * 24 * 3600],
  ['month', 30 * 24 * 3600],
  ['week', 7 * 24 * 3600],
  ['day', 24 * 3600],
  ['hour', 3600],
  ['minute', 60],
  ['second', 1],
];

/** "3 hours ago" / "in 2 days", relative to `now` (both in ms). */
export const formatRelativeTime = (
  locale: string,
  value: Date | number,
  now: number = Date.now()
): string => {
  const target = typeof value === 'number' ? value : value.getTime();
  const diffSeconds = Math.round((target - now) / 1000);
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: 'auto' });
  for (const [unit, seconds] of RELATIVE_UNITS) {
    if (Math.abs(diffSeconds) >= seconds || unit === 'second') {
      return rtf.format(Math.round(diffSeconds / seconds), unit);
    }
  }
  return rtf.format(0, 'second');
};
