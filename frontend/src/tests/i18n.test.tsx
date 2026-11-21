/**
 * Internationalisation tests (Issue #697).
 */
import React from 'react';
import fs from 'fs';
import path from 'path';
import '@testing-library/jest-dom';
import { act, fireEvent, render, screen } from '@testing-library/react';
import {
  detectLocale,
  formatCurrency,
  formatDate,
  formatNumber,
  formatRelativeTime,
  getDirection,
  i18n,
  I18nProvider,
  LanguageSelector,
  LOCALE_STORAGE_KEY,
  resolveLocale,
  resources,
  SUPPORTED_LOCALES,
} from '../i18n';
import { MintButton, ShipCard } from '../components';

const switchTo = async (locale: string) => {
  await act(async () => {
    await i18n.changeLanguage(locale);
  });
};

afterEach(async () => {
  await switchTo('en');
  localStorage.clear();
});

// ── Translation files ─────────────────────────────────────────────────────

const PLURAL_SUFFIX = /_(zero|one|two|few|many|other)$/;

const flatten = (obj: Record<string, unknown>, prefix = ''): string[] =>
  Object.entries(obj).flatMap(([key, value]) =>
    value && typeof value === 'object'
      ? flatten(value as Record<string, unknown>, `${prefix}${key}.`)
      : [`${prefix}${key}`]
  );

/** Keys with plural suffixes collapsed, since languages need different forms. */
const baseKeys = (locale: keyof typeof resources) =>
  new Set(flatten(resources[locale].translation).map((k) => k.replace(PLURAL_SUFFIX, '')));

describe('translation files', () => {
  const english = baseKeys('en');

  test.each(SUPPORTED_LOCALES.map((l) => l.code))('%s has exactly the English keys', (code) => {
    const keys = baseKeys(code as keyof typeof resources);
    expect([...english].filter((k) => !keys.has(k))).toEqual([]);
    expect([...keys].filter((k) => !english.has(k))).toEqual([]);
  });

  test('every plural key has the forms its language needs', () => {
    for (const { code } of SUPPORTED_LOCALES) {
      const categories = new Intl.PluralRules(code).resolvedOptions().pluralCategories;
      const keys = flatten(resources[code as keyof typeof resources].translation);
      const pluralBases = new Set(
        keys.filter((k) => PLURAL_SUFFIX.test(k)).map((k) => k.replace(PLURAL_SUFFIX, ''))
      );
      for (const base of pluralBases) {
        for (const category of categories) {
          expect(keys).toContain(`${base}_${category}`);
        }
      }
    }
  });

  test('interpolation placeholders match English', () => {
    const placeholders = (s: string) => (s.match(/\{\{\w+\}\}/g) ?? []).sort().join(',');
    const en = flatten(resources.en.translation);
    const get = (obj: unknown, key: string) =>
      key.split('.').reduce<unknown>((o, k) => (o as Record<string, unknown>)?.[k], obj) as
        | string
        | undefined;
    for (const { code } of SUPPORTED_LOCALES) {
      const translation = resources[code as keyof typeof resources].translation;
      for (const key of en) {
        const english = get(resources.en.translation, key) ?? '';
        const base = key.replace(PLURAL_SUFFIX, '');
        // Compare against any form of the same base key.
        const candidates = flatten(translation)
          .filter((k) => k.replace(PLURAL_SUFFIX, '') === base)
          .map((k) => get(translation, k) ?? '');
        const expected = placeholders(english).replace(/\{\{count\}\},?/, '');
        for (const candidate of candidates) {
          // Arabic "one"/"two" forms spell out the number instead of {{count}}.
          expect(placeholders(candidate).replace(/\{\{count\}\},?/, '')).toBe(expected);
        }
      }
    }
  });

  test('every literal key used in the source exists in English', () => {
    const english = new Set(flatten(resources.en.translation).map((k) => k.replace(PLURAL_SUFFIX, '')));
    const root = path.join(__dirname, '..');
    const files: string[] = [];
    const walk = (dir: string) => {
      for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) {
          if (!['tests', 'stories', 'locales'].includes(entry.name)) walk(full);
        } else if (/\.tsx?$/.test(entry.name)) files.push(full);
      }
    };
    walk(root);
    const missing: string[] = [];
    for (const file of files) {
      const source = fs.readFileSync(file, 'utf8');
      for (const match of source.matchAll(/\bt\(\s*'([a-zA-Z0-9_.]+)'/g)) {
        if (!english.has(match[1])) missing.push(`${path.relative(root, file)}: ${match[1]}`);
      }
    }
    expect(missing).toEqual([]);
  });
});

// ── Language selection, persistence and RTL ───────────────────────────────

describe('language selection', () => {
  test('the selector switches language, sets <html lang dir> and remembers the choice', () => {
    render(
      <I18nProvider>
        <LanguageSelector />
        <MintButton onMint={() => undefined} costXlm={100} />
      </I18nProvider>
    );
    const select = screen.getByLabelText('Language');
    expect(screen.getByRole('button')).toHaveTextContent('Mint Ship (100 XLM)');

    act(() => {
      fireEvent.change(select, { target: { value: 'ar' } });
    });
    expect(document.documentElement.lang).toBe('ar');
    expect(document.documentElement.dir).toBe('rtl');
    expect(localStorage.getItem(LOCALE_STORAGE_KEY)).toBe('ar');
    expect(screen.getByLabelText('اللغة')).toBeInTheDocument();
    expect(screen.getByRole('button')).toHaveTextContent('سك سفينة');

    act(() => {
      fireEvent.change(screen.getByLabelText('اللغة'), { target: { value: 'es' } });
    });
    expect(document.documentElement.dir).toBe('ltr');
    expect(screen.getByRole('button')).toHaveTextContent('Acuñar nave (100 XLM)');
  });

  test('options are labelled in their own language', () => {
    render(<LanguageSelector />);
    const option = screen.getByRole('option', { name: 'العربية' });
    expect(option).toHaveAttribute('lang', 'ar');
    expect(option).toHaveAttribute('dir', 'rtl');
  });

  test('initialLocale on the provider is applied', async () => {
    render(
      <I18nProvider initialLocale="es">
        <ShipCard
          id="s"
          name="Nova"
          shipClass="Frigate"
          rarity="Rare"
          level={2}
          stats={{ speed: 1, hull: 2, scanPower: 3 }}
        />
      </I18nProvider>
    );
    await act(async () => undefined);
    expect(screen.getByText('Fragata • Nivel 2')).toBeInTheDocument();
  });

  test('detects stored choice, then browser language, then English', () => {
    localStorage.setItem(LOCALE_STORAGE_KEY, 'es');
    expect(detectLocale()).toBe('es');
    localStorage.setItem(LOCALE_STORAGE_KEY, 'xx');
    const languages = jest.spyOn(navigator, 'languages', 'get');
    languages.mockReturnValue(['fr-FR', 'ar-EG']);
    expect(detectLocale()).toBe('ar');
    languages.mockReturnValue(['fr-FR']);
    expect(detectLocale()).toBe('en');
    languages.mockRestore();
  });

  test('locale helpers', () => {
    expect(resolveLocale('es-MX')).toBe('es');
    expect(resolveLocale('de')).toBeUndefined();
    expect(getDirection('ar-EG')).toBe('rtl');
    expect(getDirection('he')).toBe('rtl');
    expect(getDirection('en-GB')).toBe('ltr');
  });

  test('unknown keys fall back to English, then to the key', async () => {
    await switchTo('es');
    i18n.addResource('en', 'translation', 'test.onlyEnglish', 'Only English');
    expect(i18n.t('test.onlyEnglish')).toBe('Only English');
    expect(i18n.t('test.nowhere')).toBe('test.nowhere');
  });
});

// ── Locale-aware formatting ───────────────────────────────────────────────

describe('locale formatting', () => {
  test('numbers use locale separators', () => {
    expect(formatNumber('en', 1234567.5)).toBe('1,234,567.5');
    expect(formatNumber('es', 1234567.5)).toBe('1.234.567,5');
  });

  test('currency falls back for non-ISO assets', () => {
    expect(formatCurrency('en', 12.5, 'USD')).toBe('$12.50');
    expect(formatCurrency('en', 12.5, 'XLM')).toBe('12.5 XLM');
    expect(formatCurrency('es', 1234.5, 'XLM')).toBe('1234,5 XLM');
  });

  test('dates and relative times are localised', () => {
    const date = Date.UTC(2026, 0, 15, 12, 0);
    expect(formatDate('en', date, { dateStyle: 'long', timeZone: 'UTC' })).toBe('January 15, 2026');
    expect(formatDate('es', date, { dateStyle: 'long', timeZone: 'UTC' })).toBe('15 de enero de 2026');
    expect(formatRelativeTime('en', date - 3 * 3600_000, date)).toBe('3 hours ago');
    expect(formatRelativeTime('es', date + 2 * 86400_000, date)).toBe('pasado mañana');
    expect(formatRelativeTime('en', date + 5 * 86400_000, date)).toBe('in 5 days');
  });
});
