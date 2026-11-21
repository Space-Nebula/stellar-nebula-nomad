# Internationalization (i18n)

Issue #697. Source: `frontend/src/i18n/`.

The frontend uses [i18next](https://www.i18next.com/) with
[react-i18next](https://react.i18next.com/). It ships three locales:

| Code | Language | Direction |
| --- | --- | --- |
| `en` | English (default and fallback) | LTR |
| `es` | Español | LTR |
| `ar` | العربية | RTL |

## Layout

```
frontend/src/i18n/
  config.ts       i18next instance, supported locales, detection, RTL, persistence
  format.ts       Intl-based number / percent / currency / date / relative-time formatters
  react.tsx       I18nProvider, useI18n(), LanguageSelector
  index.ts        public entry point
  locales/
    en.json       source of truth for keys
    es.json
    ar.json
```

## Using it

```tsx
import { I18nProvider, LanguageSelector, useI18n } from '@stellar-nebula/frontend-components';

<I18nProvider>            {/* optional initialLocale="es" */}
  <LanguageSelector />
  <App />
</I18nProvider>

const Balance = ({ amount }: { amount: number }) => {
  const { t, formatNumber } = useI18n();
  return <p>{t('vault.loot', { amount: formatNumber(amount) })}</p>;
};
```

The instance initialises synchronously on import, so components work without
the provider (they render English). `ErrorBoundary`, a class component, uses
the exported `i18n` instance directly.

`useI18n()` returns:

* `t`: the translation function (interpolation and plurals).
* `locale` and `dir`: the active language and its direction (`'ltr'`/`'rtl'`).
* `setLocale(code)`.
* `formatNumber`, `formatPercent`, `formatCurrency`, `formatDate` and
  `formatRelativeTime`, all bound to the active locale.

## Language selection and detection

The starting locale is the first available of:

1. the user's saved choice (`localStorage['snn.locale']`);
2. the first supported language in `navigator.languages` (`es-MX` → `es`);
3. English.

`LanguageSelector` is a labelled `<select>`. Each option names the language
in that language and carries `lang`/`dir` attributes, so screen readers
pronounce it correctly. Changing the language saves the choice and updates
`<html lang dir>`. If storage is unavailable (private mode), the choice just
isn't remembered.

## RTL support

* `<html dir>` follows the active language (`ar`, `he`, `fa` and `ur` are RTL).
  `DashboardLayout` also sets `dir` on its root.
* Components use logical CSS properties (`borderInlineStart`,
  `insetInlineEnd`, `textAlign: 'start'/'end'`) instead of left and right, so
  they mirror automatically.
* `TabContainer` swaps ArrowLeft and ArrowRight in RTL, so "next" follows the
  reading direction.
* Content that must stay left-to-right, such as Stellar addresses and signed
  percentages, is marked `dir="ltr"`.

## Locale-aware formatting

Always format numbers, dates and times through `useI18n()`, never with
`toLocaleString()` or string concatenation.

* `formatNumber(1234567.5)` → `1,234,567.5` (en), `1.234.567,5` (es).
* `formatCurrency(12.5, 'USD')` → `$12.50`. Non-ISO assets such as `XLM`
  print as `12.5 XLM`, because `Intl` would otherwise show `XLM 12.50`.
* `formatRelativeTime(ts)` → "3 hours ago", "hace 2 horas", "pasado mañana".

## Plurals

Use i18next plural suffixes and pass `count`:

```json
"keys_one": "{{count}} vault key",
"keys_other": "{{count}} vault keys"
```

Each language needs the forms its CLDR plural rules define:

* English: `one`, `other`.
* Spanish: `one`, `many`, `other` (`many` is used for millions).
* Arabic: `zero`, `one`, `two`, `few`, `many`, `other`.

A test reads `Intl.PluralRules` and fails if a form is missing.

## Translation workflow

1. **Add the English string** to `locales/en.json` under a namespace that
   matches the feature (`vault.*`, `oracle.*`, `actions.*`, and so on). Keep
   keys stable and descriptive, and never key by the English text.
2. **Use it** with `t('namespace.key')`. Pass data as interpolation values
   (`{{amount}}`); never concatenate translated fragments, because word order
   differs between languages.
3. **Add every other locale.** Until a translator delivers, copy the English
   text and flag it in the PR. Missing keys fall back to English at runtime,
   but CI fails on missing keys.
4. **Run `npm test`.** `src/tests/i18n.test.tsx` checks that:
   * every locale has exactly the English keys;
   * every plural key has all the forms its language needs;
   * interpolation placeholders match English;
   * every literal `t('…')` key used in the source exists.
5. **Review RTL.** Switch to Arabic and check layout, icons and keyboard
   order.

### Adding a language

1. Create `locales/<code>.json` from `en.json`.
2. Register it in `config.ts`: add it to `SUPPORTED_LOCALES` (with
   `nativeName` and `dir`), `resources` and the `Locale` type.
3. If it is RTL and not yet listed, add it to `RTL_LANGUAGES`.
4. Run the tests. The parity and plural checks run for every registered locale.

## Coverage

Translated: the shared component library (layout, feedback, input, action and
display components), `LanguageSelector`, `MarketOracleDashboard` and
`TreasureVaultPanel`.

Not yet translated: the large feature views (`Marketplace`, `GuildManagement`,
`AnalyticsDashboard`, `ShipManagement`, `Tournaments`, the analytics panels,
`GuildQuestBoard`, `ContentEditor` and similar) still contain inline English.
Move them over with the workflow above, one view per PR. The source-scan test
validates their keys as soon as they use `t()`.
