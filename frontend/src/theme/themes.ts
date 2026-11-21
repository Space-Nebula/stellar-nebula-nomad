/**
 * Theme definitions, resolution, custom colours and persistence (Issue #700).
 *
 * A theme is a full set of the colour tokens in `a11y/colors`. Every theme is
 * checked against WCAG 2.1 (AA for normal themes, AAA 7:1 for the
 * high-contrast themes) by `src/tests/theme.test.tsx`, and custom colours are
 * only accepted when the result still passes, so no combination of settings
 * can produce unreadable text.
 *
 * The on-chain counterpart is `src/theme_customizer.rs`
 * (`ThemePreferences`); `toContractPreferences` / `fromContractPreferences`
 * convert between the two.
 */
import { colors, contrastRatio } from '../a11y';

// ── Tokens ─────────────────────────────────────────────────────────────────

export type ThemeTokens = { [K in keyof typeof colors]: string };
export type ThemeTokenName = keyof ThemeTokens;
export type ColorScheme = 'dark' | 'light';

export const THEME_TOKEN_NAMES = Object.keys(colors) as ThemeTokenName[];

/** Theme families; each has a dark and a light palette. */
export const THEME_FAMILIES = ['nebula', 'cosmic', 'aurora'] as const;
export type ThemeFamily = (typeof THEME_FAMILIES)[number];

export type ThemeMode = 'dark' | 'light' | 'system';
export const THEME_MODES: ThemeMode[] = ['dark', 'light', 'system'];

/** Mirrors `theme_customizer::AccessibilityProfile`. */
export const ACCESSIBILITY_PROFILES = [
  'standard',
  'highContrast',
  'redGreenSafe',
  'blueYellowSafe',
] as const;
export type AccessibilityProfile = (typeof ACCESSIBILITY_PROFILES)[number];

/** Mirrors `theme_customizer::ColorSlot`; maps each slot to the token it overrides. */
export const COLOR_SLOTS = {
  background: 'bgApp',
  surface: 'bgSurface',
  text: 'textPrimary',
  primary: 'primaryFill',
  accent: 'accent',
  border: 'border',
} as const satisfies Record<string, ThemeTokenName>;
export type ColorSlot = keyof typeof COLOR_SLOTS;
export const COLOR_SLOT_NAMES = Object.keys(COLOR_SLOTS) as ColorSlot[];

// ── Palettes ───────────────────────────────────────────────────────────────

const shared = {
  textOnAccent: '#FFFFFF',
  secondaryFill: '#6D28D9',
  successFill: '#047857',
  bondFill: '#BE185D',
  selectedFill: '#4F46E5',
};

export const PALETTES: Record<ThemeFamily, Record<ColorScheme, ThemeTokens>> = {
  nebula: {
    // The original component-library palette.
    dark: { ...colors },
    light: {
      ...shared,
      bgApp: '#F1F5F9',
      bgSurface: '#FFFFFF',
      bgRaised: '#F8FAFC',
      border: '#CBD5E1',
      textPrimary: '#0F172A',
      textSecondary: '#334155',
      textMuted: '#475569',
      textOnWarning: '#0F172A',
      accent: '#0369A1',
      success: '#047857',
      danger: '#B91C1C',
      warning: '#92400E',
      epic: '#7E22CE',
      primaryFill: '#1D4ED8',
      dangerFill: '#B91C1C',
      warningFill: '#F59E0B',
      focus: '#1D4ED8',
    },
  },
  cosmic: {
    dark: {
      ...shared,
      bgApp: '#120A24',
      bgSurface: '#1A1033',
      bgRaised: '#261A47',
      border: '#4C3A7A',
      textPrimary: '#F5F3FF',
      textSecondary: '#DDD6FE',
      textMuted: '#A99BD6',
      textOnWarning: '#1A1033',
      accent: '#F0ABFC',
      success: '#6EE7B7',
      danger: '#FCA5A5',
      warning: '#FCD34D',
      epic: '#D8B4FE',
      primaryFill: '#7C3AED',
      secondaryFill: '#A21CAF',
      dangerFill: '#B91C1C',
      warningFill: '#FBBF24',
      focus: '#F0ABFC',
    },
    light: {
      ...shared,
      bgApp: '#F5F0FF',
      bgSurface: '#FFFFFF',
      bgRaised: '#FAF5FF',
      border: '#D8C7F5',
      textPrimary: '#1E1033',
      textSecondary: '#3B2A5C',
      textMuted: '#5B4A7A',
      textOnWarning: '#1E1033',
      accent: '#86198F',
      success: '#047857',
      danger: '#B91C1C',
      warning: '#92400E',
      epic: '#6D28D9',
      primaryFill: '#6D28D9',
      secondaryFill: '#86198F',
      dangerFill: '#B91C1C',
      warningFill: '#F59E0B',
      focus: '#86198F',
    },
  },
  aurora: {
    dark: {
      ...shared,
      bgApp: '#04151A',
      bgSurface: '#072227',
      bgRaised: '#0D3238',
      border: '#1F5560',
      textPrimary: '#ECFEFF',
      textSecondary: '#CFFAFE',
      textMuted: '#8FC1C9',
      textOnWarning: '#04151A',
      accent: '#67E8F9',
      success: '#6EE7B7',
      danger: '#FCA5A5',
      warning: '#FDE68A',
      epic: '#C4B5FD',
      primaryFill: '#0E7490',
      dangerFill: '#B91C1C',
      warningFill: '#FBBF24',
      focus: '#67E8F9',
    },
    light: {
      ...shared,
      bgApp: '#ECFDF9',
      bgSurface: '#FFFFFF',
      bgRaised: '#F0FDFA',
      border: '#A7E3D8',
      textPrimary: '#042F2E',
      textSecondary: '#134E4A',
      textMuted: '#2D5F5A',
      textOnWarning: '#042F2E',
      accent: '#0F766E',
      success: '#047857',
      danger: '#B91C1C',
      warning: '#92400E',
      epic: '#6D28D9',
      primaryFill: '#0F766E',
      dangerFill: '#B91C1C',
      warningFill: '#F59E0B',
      focus: '#0F766E',
    },
  },
};

/**
 * High-contrast palettes (WCAG AAA: 7:1 for text). They replace the family
 * palette entirely. `border` doubles as the secondary-button fill, so it is
 * tuned for text contrast; outlines rely on the yellow/blue focus ring.
 */
export const HIGH_CONTRAST: Record<ColorScheme, ThemeTokens> = {
  dark: {
    bgApp: '#000000',
    bgSurface: '#000000',
    bgRaised: '#0A0A0A',
    border: '#525252',
    textPrimary: '#FFFFFF',
    textSecondary: '#FFFFFF',
    textMuted: '#E5E5E5',
    textOnAccent: '#FFFFFF',
    textOnWarning: '#000000',
    accent: '#7DD3FC',
    success: '#4ADE80',
    danger: '#FF8A8A',
    warning: '#FFE14D',
    epic: '#E9B8FF',
    primaryFill: '#0033CC',
    secondaryFill: '#5B21B6',
    successFill: '#035E3F',
    dangerFill: '#9B1C1C',
    warningFill: '#FFE14D',
    bondFill: '#8A1043',
    selectedFill: '#3730A3',
    focus: '#FFE14D',
  },
  light: {
    bgApp: '#FFFFFF',
    bgSurface: '#FFFFFF',
    bgRaised: '#F2F2F2',
    border: '#B3B3B3',
    textPrimary: '#000000',
    textSecondary: '#1A1A1A',
    textMuted: '#333333',
    textOnAccent: '#FFFFFF',
    textOnWarning: '#000000',
    accent: '#00468C',
    success: '#005C2E',
    danger: '#A30000',
    warning: '#6B3E00',
    epic: '#5B1A8F',
    primaryFill: '#0033CC',
    secondaryFill: '#5B21B6',
    successFill: '#005C2E',
    dangerFill: '#A30000',
    warningFill: '#FFD60A',
    bondFill: '#8A1043',
    selectedFill: '#3730A3',
    focus: '#0033CC',
  },
};

/**
 * Colour-vision overlays: replace the status colours that collide for the
 * given deficiency, keeping the family's surfaces.
 *
 * - red/green (deuteranopia, protanopia): success → blue, danger → orange,
 *   warning → yellow (Okabe–Ito hues).
 * - blue/yellow (tritanopia): success → teal, danger → red, warning → pink,
 *   accent → magenta.
 */
export const COLOR_VISION_OVERLAYS: Record<
  'redGreenSafe' | 'blueYellowSafe',
  Record<ColorScheme, Partial<ThemeTokens>>
> = {
  redGreenSafe: {
    dark: {
      success: '#56B4E9',
      danger: '#F5A65B',
      warning: '#F0E442',
      successFill: '#005A9C',
      dangerFill: '#A34700',
      warningFill: '#F0E442',
    },
    light: {
      success: '#005A9C',
      danger: '#A34700',
      warning: '#665C00',
      successFill: '#005A9C',
      dangerFill: '#A34700',
      warningFill: '#F0E442',
    },
  },
  blueYellowSafe: {
    dark: {
      accent: '#F9A8D4',
      success: '#5EEAD4',
      danger: '#FF8A8A',
      warning: '#FFB4C6',
      successFill: '#0F766E',
      dangerFill: '#B91C1C',
      warningFill: '#FFB4C6',
    },
    light: {
      accent: '#9D174D',
      success: '#0F766E',
      danger: '#B91C1C',
      warning: '#9F1239',
      successFill: '#0F766E',
      dangerFill: '#B91C1C',
      warningFill: '#FDA4AF',
    },
  },
};

// ── Contrast checks ────────────────────────────────────────────────────────

export const AA_TEXT = 4.5;
export const AAA_TEXT = 7;
export const NON_TEXT = 3;

const TEXT_TOKENS: ThemeTokenName[] = [
  'textPrimary',
  'textSecondary',
  'textMuted',
  'accent',
  'success',
  'danger',
  'warning',
  'epic',
];
const SURFACE_TOKENS: ThemeTokenName[] = ['bgApp', 'bgSurface', 'bgRaised'];
const FILL_TOKENS: ThemeTokenName[] = [
  'primaryFill',
  'secondaryFill',
  'successFill',
  'dangerFill',
  'bondFill',
  'selectedFill',
];

export interface ContrastFailure {
  foreground: ThemeTokenName;
  background: ThemeTokenName;
  ratio: number;
  required: number;
}

/**
 * Every token pair the components rely on that falls below `minText`
 * (text) or 3:1 (focus ring). Empty means the theme is safe.
 */
export const contrastFailures = (theme: ThemeTokens, minText = AA_TEXT): ContrastFailure[] => {
  const failures: ContrastFailure[] = [];
  const check = (fg: ThemeTokenName, bg: ThemeTokenName, required: number) => {
    const ratio = contrastRatio(theme[fg], theme[bg]);
    if (ratio < required) failures.push({ foreground: fg, background: bg, ratio, required });
  };
  for (const bg of SURFACE_TOKENS) {
    for (const fg of TEXT_TOKENS) check(fg, bg, minText);
    check('focus', bg, NON_TEXT);
  }
  for (const fill of FILL_TOKENS) check('textOnAccent', fill, minText);
  check('textOnWarning', 'warningFill', minText);
  // `border` is also the secondary-button fill.
  check('textPrimary', 'border', minText);
  return failures;
};

// ── Preferences ────────────────────────────────────────────────────────────

export interface ThemePreferences {
  family: ThemeFamily;
  mode: ThemeMode;
  accessibility: AccessibilityProfile;
  /** `#RRGGBB` per overridden slot. */
  customColors: Partial<Record<ColorSlot, string>>;
  reducedMotion: boolean;
  /** Percent of the default font size (75–200). */
  fontScale: number;
}

export const DEFAULT_PREFERENCES: ThemePreferences = {
  family: 'nebula',
  mode: 'system',
  accessibility: 'standard',
  customColors: {},
  reducedMotion: false,
  fontScale: 100,
};

export const FONT_SCALES = [75, 100, 125, 150, 175, 200];
export const MIN_FONT_SCALE = 75;
export const MAX_FONT_SCALE = 200;

/** What the device reports through media queries. */
export interface SystemPreferences {
  prefersDark: boolean;
  prefersMoreContrast: boolean;
  prefersReducedMotion: boolean;
}

export const DEFAULT_SYSTEM: SystemPreferences = {
  prefersDark: true,
  prefersMoreContrast: false,
  prefersReducedMotion: false,
};

export const HEX_COLOR = /^#[0-9A-Fa-f]{6}$/;

export const normalizeHex = (value: string): string | undefined => {
  const trimmed = value.trim();
  const short = /^#?([0-9A-Fa-f])([0-9A-Fa-f])([0-9A-Fa-f])$/.exec(trimmed);
  if (short) return `#${short[1]}${short[1]}${short[2]}${short[2]}${short[3]}${short[3]}`.toUpperCase();
  const long = trimmed.startsWith('#') ? trimmed : `#${trimmed}`;
  return HEX_COLOR.test(long) ? long.toUpperCase() : undefined;
};

/** Coerce untrusted input (storage, chain, URL) into valid preferences. */
export const sanitizePreferences = (input: unknown): ThemePreferences => {
  const raw = (input && typeof input === 'object' ? input : {}) as Record<string, unknown>;
  const pick = <T extends string>(value: unknown, allowed: readonly T[], fallback: T): T =>
    allowed.includes(value as T) ? (value as T) : fallback;

  const customColors: Partial<Record<ColorSlot, string>> = {};
  if (raw.customColors && typeof raw.customColors === 'object') {
    for (const [slot, value] of Object.entries(raw.customColors as Record<string, unknown>)) {
      if (!(slot in COLOR_SLOTS) || typeof value !== 'string') continue;
      const hex = normalizeHex(value);
      if (hex) customColors[slot as ColorSlot] = hex;
    }
  }
  const scale = Number(raw.fontScale);
  return {
    family: pick(raw.family, THEME_FAMILIES, DEFAULT_PREFERENCES.family),
    mode: pick(raw.mode, THEME_MODES, DEFAULT_PREFERENCES.mode),
    accessibility: pick(raw.accessibility, ACCESSIBILITY_PROFILES, DEFAULT_PREFERENCES.accessibility),
    customColors,
    reducedMotion: raw.reducedMotion === true,
    fontScale: Number.isFinite(scale)
      ? Math.min(MAX_FONT_SCALE, Math.max(MIN_FONT_SCALE, Math.round(scale)))
      : DEFAULT_PREFERENCES.fontScale,
  };
};

// ── Resolution ─────────────────────────────────────────────────────────────

export interface ResolvedTheme {
  /** Stable identifier, e.g. `aurora-light` or `high-contrast-dark`. */
  id: string;
  scheme: ColorScheme;
  highContrast: boolean;
  tokens: ThemeTokens;
  /** Minimum text contrast this theme guarantees. */
  minTextContrast: number;
  /** Custom colours that were refused because they broke contrast. */
  rejectedSlots: ColorSlot[];
  reducedMotion: boolean;
  fontScale: number;
}

export const effectiveScheme = (mode: ThemeMode, system: SystemPreferences): ColorScheme =>
  mode === 'system' ? (system.prefersDark ? 'dark' : 'light') : mode;

/**
 * Apply custom colours one by one, keeping each only if the theme still meets
 * `minText`. Order follows `COLOR_SLOT_NAMES` so results are deterministic.
 */
export const applyCustomColors = (
  base: ThemeTokens,
  customColors: Partial<Record<ColorSlot, string>>,
  minText = AA_TEXT
): { tokens: ThemeTokens; rejected: ColorSlot[] } => {
  let tokens = { ...base };
  const rejected: ColorSlot[] = [];
  for (const slot of COLOR_SLOT_NAMES) {
    const value = customColors[slot];
    if (!value) continue;
    const hex = normalizeHex(value);
    const candidate = hex ? { ...tokens, [COLOR_SLOTS[slot]]: hex } : undefined;
    if (candidate && contrastFailures(candidate, minText).length === 0) {
      tokens = candidate;
    } else {
      rejected.push(slot);
    }
  }
  return { tokens, rejected };
};

/** Turn preferences + device settings into concrete tokens. */
export const resolveTheme = (
  prefs: ThemePreferences,
  system: SystemPreferences = DEFAULT_SYSTEM
): ResolvedTheme => {
  const scheme = effectiveScheme(prefs.mode, system);
  // The device's "more contrast" request is honoured unless the player
  // explicitly picked a colour-vision palette.
  const highContrast =
    prefs.accessibility === 'highContrast' ||
    (prefs.accessibility === 'standard' && system.prefersMoreContrast);

  let base: ThemeTokens;
  let id: string;
  if (highContrast) {
    base = HIGH_CONTRAST[scheme];
    id = `high-contrast-${scheme}`;
  } else {
    base = PALETTES[prefs.family][scheme];
    id = `${prefs.family}-${scheme}`;
    if (prefs.accessibility === 'redGreenSafe' || prefs.accessibility === 'blueYellowSafe') {
      base = { ...base, ...COLOR_VISION_OVERLAYS[prefs.accessibility][scheme] };
      id = `${id}-${prefs.accessibility}`;
    }
  }

  const minTextContrast = highContrast ? AAA_TEXT : AA_TEXT;
  const { tokens, rejected } = applyCustomColors(base, prefs.customColors, minTextContrast);
  return {
    id,
    scheme,
    highContrast,
    tokens,
    minTextContrast,
    rejectedSlots: rejected,
    reducedMotion: prefs.reducedMotion || system.prefersReducedMotion,
    fontScale: prefs.fontScale,
  };
};

// ── CSS variables ──────────────────────────────────────────────────────────

const kebab = (s: string) => s.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);

/** CSS custom property name for a token, e.g. `--snn-bg-surface`. */
export const cssVarName = (token: ThemeTokenName) => `--snn-${kebab(token)}`;

export const themeToCssVariables = (theme: ResolvedTheme): Record<string, string> => {
  const vars: Record<string, string> = {};
  for (const token of THEME_TOKEN_NAMES) vars[cssVarName(token)] = theme.tokens[token];
  vars['--snn-font-scale'] = String(theme.fontScale / 100);
  return vars;
};

/**
 * Token references for inline styles: `var(--snn-*, <nebula-dark fallback>)`.
 * Components styled with these follow the active theme and still render with
 * the default palette when no `ThemeProvider` is mounted.
 */
export const themeColors = Object.fromEntries(
  THEME_TOKEN_NAMES.map((token) => [token, `var(${cssVarName(token)}, ${colors[token]})`])
) as ThemeTokens;

// ── Persistence ────────────────────────────────────────────────────────────

export const THEME_STORAGE_KEY = 'snn.theme';

type StorageLike = Pick<Storage, 'getItem' | 'setItem' | 'removeItem'>;

const defaultStorage = (): StorageLike | undefined => {
  try {
    return typeof window !== 'undefined' ? window.localStorage : undefined;
  } catch {
    return undefined; // Storage can throw in privacy modes.
  }
};

export const loadThemePreferences = (
  storage: StorageLike | undefined = defaultStorage()
): ThemePreferences => {
  try {
    const raw = storage?.getItem(THEME_STORAGE_KEY);
    return raw ? sanitizePreferences(JSON.parse(raw)) : { ...DEFAULT_PREFERENCES };
  } catch {
    return { ...DEFAULT_PREFERENCES };
  }
};

export const saveThemePreferences = (
  prefs: ThemePreferences,
  storage: StorageLike | undefined = defaultStorage()
): void => {
  try {
    storage?.setItem(THEME_STORAGE_KEY, JSON.stringify(sanitizePreferences(prefs)));
  } catch {
    // Quota or privacy-mode errors: the theme still applies for this session.
  }
};

export const clearThemePreferences = (storage: StorageLike | undefined = defaultStorage()) => {
  try {
    storage?.removeItem(THEME_STORAGE_KEY);
  } catch {
    // ignore
  }
};

// ── Contract mapping ───────────────────────────────────────────────────────

/** Shape of `theme_customizer::ThemePreferences` as returned by the SDK. */
export interface ContractThemePreferences {
  theme_id: string;
  /** 0 dark, 1 light, 2 system. */
  mode: number;
  /** 0 standard, 1 high contrast, 2 red/green safe, 3 blue/yellow safe. */
  accessibility: number;
  custom_colors: { slot: number; rgb: number }[];
  reduced_motion: boolean;
  font_scale_pct: number;
}

/** Family ↔ on-chain preset (`generate_theme_preview` names match). */
export const FAMILY_TO_CONTRACT_THEME: Record<ThemeFamily, string> = {
  nebula: 'nebula4', // Quasar
  cosmic: 'nebula1', // Cosmic
  aurora: 'nebula8', // Aurora
};

const CONTRACT_MODES: ThemeMode[] = ['dark', 'light', 'system'];

export const toContractPreferences = (prefs: ThemePreferences): ContractThemePreferences => ({
  theme_id: FAMILY_TO_CONTRACT_THEME[prefs.family],
  mode: CONTRACT_MODES.indexOf(prefs.mode),
  accessibility: ACCESSIBILITY_PROFILES.indexOf(prefs.accessibility),
  custom_colors: COLOR_SLOT_NAMES.flatMap((slot, index) => {
    const hex = prefs.customColors[slot] && normalizeHex(prefs.customColors[slot]!);
    return hex ? [{ slot: index, rgb: parseInt(hex.slice(1), 16) }] : [];
  }),
  reduced_motion: prefs.reducedMotion,
  font_scale_pct: prefs.fontScale,
});

export const fromContractPreferences = (value: ContractThemePreferences): ThemePreferences => {
  const family =
    (Object.entries(FAMILY_TO_CONTRACT_THEME).find(([, id]) => id === value.theme_id)?.[0] as
      | ThemeFamily
      | undefined) ?? 'nebula';
  const customColors: Partial<Record<ColorSlot, string>> = {};
  for (const { slot, rgb } of value.custom_colors ?? []) {
    const name = COLOR_SLOT_NAMES[slot];
    if (name && rgb >= 0 && rgb <= 0xffffff) {
      customColors[name] = `#${rgb.toString(16).padStart(6, '0').toUpperCase()}`;
    }
  }
  return sanitizePreferences({
    family,
    mode: CONTRACT_MODES[value.mode],
    accessibility: ACCESSIBILITY_PROFILES[value.accessibility],
    customColors,
    reducedMotion: value.reduced_motion,
    fontScale: value.font_scale_pct,
  });
};
