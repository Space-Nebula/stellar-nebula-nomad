/**
 * Theme context (Issue #700).
 *
 *   <ThemeProvider>
 *     <ThemeSettingsPanel />
 *     ...
 *   </ThemeProvider>
 *
 * The provider resolves the player's preferences against the device's media
 * queries, writes the tokens as CSS custom properties on `<html>` (so any
 * component styled with `themeColors` follows along), and persists every
 * change to localStorage. Pass `onPreferencesChange` to also save the
 * preferences on-chain (`set_theme_preferences`).
 */
import React, { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';

import {
  ColorSlot,
  DEFAULT_PREFERENCES,
  DEFAULT_SYSTEM,
  loadThemePreferences,
  normalizeHex,
  ResolvedTheme,
  resolveTheme,
  sanitizePreferences,
  saveThemePreferences,
  SystemPreferences,
  themeToCssVariables,
  ThemePreferences,
} from './themes';

export interface ThemeContextValue {
  preferences: ThemePreferences;
  theme: ResolvedTheme;
  system: SystemPreferences;
  setPreferences: (update: Partial<ThemePreferences>) => void;
  /**
   * Set (or with `undefined`, clear) one custom colour. Returns `false` when
   * the colour is malformed or would break contrast; it is then not saved.
   */
  setCustomColor: (slot: ColorSlot, value: string | undefined) => boolean;
  resetPreferences: () => void;
}

const ThemeContext = createContext<ThemeContextValue | undefined>(undefined);

const QUERIES = {
  prefersDark: '(prefers-color-scheme: dark)',
  prefersMoreContrast: '(prefers-contrast: more)',
  prefersReducedMotion: '(prefers-reduced-motion: reduce)',
} as const;

const readSystem = (): SystemPreferences => {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') {
    return DEFAULT_SYSTEM;
  }
  return {
    prefersDark: window.matchMedia(QUERIES.prefersDark).matches,
    prefersMoreContrast: window.matchMedia(QUERIES.prefersMoreContrast).matches,
    prefersReducedMotion: window.matchMedia(QUERIES.prefersReducedMotion).matches,
  };
};

export interface ThemeProviderProps {
  /** Starting preferences (e.g. loaded from the chain). Defaults to localStorage. */
  initialPreferences?: Partial<ThemePreferences>;
  /** Override media-query detection (tests, SSR). */
  system?: SystemPreferences;
  /** Disable writing CSS variables to `<html>` (e.g. when nesting providers). */
  applyToDocument?: boolean;
  /** Called after every change, for on-chain persistence. */
  onPreferencesChange?: (prefs: ThemePreferences) => void;
  children?: React.ReactNode;
}

export const ThemeProvider: React.FC<ThemeProviderProps> = ({
  initialPreferences,
  system: systemOverride,
  applyToDocument = true,
  onPreferencesChange,
  children,
}) => {
  const [preferences, setPrefsState] = useState<ThemePreferences>(() =>
    initialPreferences
      ? sanitizePreferences({ ...DEFAULT_PREFERENCES, ...initialPreferences })
      : loadThemePreferences()
  );
  const [detected, setDetected] = useState<SystemPreferences>(readSystem);
  const system = systemOverride ?? detected;

  // Follow live changes to the device settings.
  useEffect(() => {
    if (systemOverride || typeof window === 'undefined' || !window.matchMedia) return;
    const lists = Object.values(QUERIES).map((q) => window.matchMedia(q));
    const update = () => setDetected(readSystem());
    lists.forEach((l) => l.addEventListener?.('change', update));
    return () => lists.forEach((l) => l.removeEventListener?.('change', update));
  }, [systemOverride]);

  const theme = useMemo(() => resolveTheme(preferences, system), [preferences, system]);

  useEffect(() => {
    if (!applyToDocument || typeof document === 'undefined') return;
    const root = document.documentElement;
    for (const [name, value] of Object.entries(themeToCssVariables(theme))) {
      root.style.setProperty(name, value);
    }
    root.style.colorScheme = theme.scheme;
    root.style.fontSize = `${theme.fontScale}%`;
    root.dataset.theme = theme.id;
    root.dataset.reducedMotion = String(theme.reducedMotion);
  }, [theme, applyToDocument]);

  const commit = useCallback(
    (next: ThemePreferences) => {
      const clean = sanitizePreferences(next);
      setPrefsState(clean);
      saveThemePreferences(clean);
      onPreferencesChange?.(clean);
    },
    [onPreferencesChange]
  );

  const setPreferences = useCallback(
    (update: Partial<ThemePreferences>) => commit({ ...preferences, ...update }),
    [commit, preferences]
  );

  const setCustomColor = useCallback(
    (slot: ColorSlot, value: string | undefined) => {
      const customColors = { ...preferences.customColors };
      if (value === undefined) {
        delete customColors[slot];
        commit({ ...preferences, customColors });
        return true;
      }
      const hex = normalizeHex(value);
      if (!hex) return false;
      customColors[slot] = hex;
      const candidate = { ...preferences, customColors };
      if (resolveTheme(candidate, system).rejectedSlots.includes(slot)) return false;
      commit(candidate);
      return true;
    },
    [commit, preferences, system]
  );

  const resetPreferences = useCallback(() => commit({ ...DEFAULT_PREFERENCES }), [commit]);

  const value = useMemo(
    () => ({ preferences, theme, system, setPreferences, setCustomColor, resetPreferences }),
    [preferences, theme, system, setPreferences, setCustomColor, resetPreferences]
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
};

/** Access the active theme. Must be used inside `ThemeProvider`. */
export const useTheme = (): ThemeContextValue => {
  const value = useContext(ThemeContext);
  if (!value) throw new Error('useTheme must be used inside <ThemeProvider>');
  return value;
};
