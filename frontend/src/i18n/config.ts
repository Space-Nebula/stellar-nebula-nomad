/**
 * i18next setup (Issue #697).
 *
 * The instance is initialised synchronously on import with bundled
 * resources, so components can call `useTranslation()` even when an app has
 * not rendered `<I18nProvider>`. The provider is still recommended: it scopes
 * the instance and lets apps pass an initial locale.
 */
import i18next, { i18n as I18nInstance } from 'i18next';
import { initReactI18next } from 'react-i18next';

import en from './locales/en.json';
import es from './locales/es.json';
import ar from './locales/ar.json';

export type TextDirection = 'ltr' | 'rtl';

export interface LocaleInfo {
  /** BCP 47 language tag. */
  code: string;
  /** Name of the language in that language (shown in the selector). */
  nativeName: string;
  dir: TextDirection;
}

/** Every locale shipped with the app. Add new locales here and in `resources`. */
export const SUPPORTED_LOCALES: readonly LocaleInfo[] = [
  { code: 'en', nativeName: 'English', dir: 'ltr' },
  { code: 'es', nativeName: 'Español', dir: 'ltr' },
  { code: 'ar', nativeName: 'العربية', dir: 'rtl' },
] as const;

export type Locale = 'en' | 'es' | 'ar';

export const DEFAULT_LOCALE: Locale = 'en';
export const LOCALE_STORAGE_KEY = 'snn.locale';

export const resources = {
  en: { translation: en },
  es: { translation: es },
  ar: { translation: ar },
} as const;

const RTL_LANGUAGES = new Set(['ar', 'he', 'fa', 'ur']);

/** Text direction for a language tag (`ar-EG` → `rtl`). */
export const getDirection = (locale: string): TextDirection =>
  RTL_LANGUAGES.has(locale.split('-')[0].toLowerCase()) ? 'rtl' : 'ltr';

export const isRtlLocale = (locale: string): boolean => getDirection(locale) === 'rtl';

/** Map any language tag onto a supported locale (`es-MX` → `es`). */
export const resolveLocale = (candidate: string | null | undefined): Locale | undefined => {
  if (!candidate) return undefined;
  const base = candidate.split('-')[0].toLowerCase();
  return SUPPORTED_LOCALES.some((l) => l.code === base) ? (base as Locale) : undefined;
};

const readStoredLocale = (): Locale | undefined => {
  try {
    return resolveLocale(globalThis.localStorage?.getItem(LOCALE_STORAGE_KEY));
  } catch {
    return undefined; // storage blocked (private mode, sandboxed iframe)
  }
};

const storeLocale = (locale: string): void => {
  try {
    globalThis.localStorage?.setItem(LOCALE_STORAGE_KEY, locale);
  } catch {
    /* non-fatal: the choice just isn't remembered */
  }
};

/** Stored choice → browser preference → default. */
export const detectLocale = (): Locale => {
  const stored = readStoredLocale();
  if (stored) return stored;
  const preferred =
    typeof navigator !== 'undefined' ? navigator.languages ?? [navigator.language] : [];
  for (const tag of preferred) {
    const match = resolveLocale(tag);
    if (match) return match;
  }
  return DEFAULT_LOCALE;
};

/** Reflect the active language on `<html lang dir>` for screen readers and CSS. */
export const applyDocumentLanguage = (locale: string): void => {
  if (typeof document === 'undefined') return;
  document.documentElement.lang = locale;
  document.documentElement.dir = getDirection(locale);
};

export const i18n: I18nInstance = i18next.createInstance();

i18n.use(initReactI18next).init({
  resources,
  lng: detectLocale(),
  fallbackLng: DEFAULT_LOCALE,
  supportedLngs: SUPPORTED_LOCALES.map((l) => l.code),
  interpolation: { escapeValue: false }, // React already escapes
  returnNull: false,
  initImmediate: false, // resources are bundled: initialise synchronously
});

applyDocumentLanguage(i18n.language);
i18n.on('languageChanged', (locale) => {
  applyDocumentLanguage(locale);
  storeLocale(locale);
});

export default i18n;
