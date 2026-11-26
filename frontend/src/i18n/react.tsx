/**
 * Internationalisation entry point (Issue #697).
 *
 *   <I18nProvider initialLocale="es">
 *     <LanguageSelector />
 *     ...
 *   </I18nProvider>
 *
 * Inside components use `useI18n()` for `t`, the active locale/direction and
 * locale-aware formatters.
 */
import React, { useEffect, useId, useMemo } from 'react';
import { I18nextProvider, useTranslation } from 'react-i18next';

import i18n, {
  getDirection,
  Locale,
  resolveLocale,
  SUPPORTED_LOCALES,
  TextDirection,
} from './config';
import {
  formatCurrency,
  formatDate,
  formatNumber,
  formatPercent,
  formatRelativeTime,
} from './format';
import { colors } from '../a11y';

export {
  i18n,
  SUPPORTED_LOCALES,
  DEFAULT_LOCALE,
  LOCALE_STORAGE_KEY,
  getDirection,
  isRtlLocale,
  resolveLocale,
  detectLocale,
  resources,
} from './config';
export type { Locale, LocaleInfo, TextDirection } from './config';
export * from './format';

export interface I18nProviderProps {
  /** Force a starting locale (otherwise: stored choice → browser → English). */
  initialLocale?: Locale;
  children?: React.ReactNode;
}

export const I18nProvider: React.FC<I18nProviderProps> = ({ initialLocale, children }) => {
  useEffect(() => {
    if (initialLocale && i18n.language !== initialLocale) {
      void i18n.changeLanguage(initialLocale);
    }
  }, [initialLocale]);
  return <I18nextProvider i18n={i18n}>{children}</I18nextProvider>;
};

export interface I18nHelpers {
  t: ReturnType<typeof useTranslation>['t'];
  locale: Locale;
  dir: TextDirection;
  setLocale: (locale: Locale) => Promise<void>;
  formatNumber: (value: number, options?: Intl.NumberFormatOptions) => string;
  formatPercent: (ratio: number, options?: Intl.NumberFormatOptions) => string;
  formatCurrency: (value: number, currency: string) => string;
  formatDate: (value: Date | number, options?: Intl.DateTimeFormatOptions) => string;
  formatRelativeTime: (value: Date | number, now?: number) => string;
}

/** Translation function, active locale and bound locale-aware formatters. */
export const useI18n = (): I18nHelpers => {
  const { t, i18n: instance } = useTranslation(undefined, { i18n });
  const locale = (resolveLocale(instance.language) ?? 'en') as Locale;
  return useMemo(
    () => ({
      t,
      locale,
      dir: getDirection(locale),
      setLocale: async (next: Locale) => {
        await instance.changeLanguage(next);
      },
      formatNumber: (value, options) => formatNumber(locale, value, options),
      formatPercent: (ratio, options) => formatPercent(locale, ratio, options),
      formatCurrency: (value, currency) => formatCurrency(locale, value, currency),
      formatDate: (value, options) => formatDate(locale, value, options),
      formatRelativeTime: (value, now) => formatRelativeTime(locale, value, now),
    }),
    [t, locale, instance]
  );
};

/** Accessible language picker; each option is labelled in its own language. */
export const LanguageSelector: React.FC = () => {
  const { t, locale, setLocale } = useI18n();
  const id = useId();
  return (
    <div style={{ display: 'inline-flex', alignItems: 'center', gap: 8 }}>
      <label htmlFor={id} style={{ color: colors.textMuted, fontSize: '0.85rem' }}>
        {t('common.language')}
      </label>
      <select
        id={id}
        value={locale}
        onChange={(event) => void setLocale(event.target.value as Locale)}
        style={{
          backgroundColor: colors.bgSurface,
          color: colors.textPrimary,
          border: `1px solid ${colors.textMuted}`,
          borderRadius: 6,
          padding: '4px 8px',
        }}
      >
        {SUPPORTED_LOCALES.map((l) => (
          <option key={l.code} value={l.code} lang={l.code} dir={l.dir}>
            {l.nativeName}
          </option>
        ))}
      </select>
    </div>
  );
};
