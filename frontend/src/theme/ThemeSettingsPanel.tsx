/**
 * Appearance settings (Issue #700): theme family, dark/light/system mode,
 * accessibility palette, custom colours with live contrast checking, reduced
 * motion and text size. Must be rendered inside `ThemeProvider`.
 */
import React, { useId, useState } from 'react';

import { useI18n } from '../i18n';
import { useTheme } from './ThemeProvider';
import {
  ACCESSIBILITY_PROFILES,
  AccessibilityProfile,
  COLOR_SLOT_NAMES,
  COLOR_SLOTS,
  ColorSlot,
  FONT_SCALES,
  PALETTES,
  THEME_FAMILIES,
  THEME_MODES,
  ThemeMode,
  themeColors as c,
} from './themes';

const fieldsetStyle: React.CSSProperties = {
  border: `1px solid ${c.border}`,
  borderRadius: 10,
  padding: 12,
  margin: 0,
  display: 'flex',
  flexWrap: 'wrap',
  gap: 10,
};

const legendStyle: React.CSSProperties = { fontWeight: 700, padding: '0 6px' };

const choiceStyle = (selected: boolean): React.CSSProperties => ({
  display: 'inline-flex',
  alignItems: 'center',
  gap: 8,
  padding: '6px 10px',
  borderRadius: 8,
  border: `2px solid ${selected ? c.focus : c.border}`,
  cursor: 'pointer',
});

const buttonStyle: React.CSSProperties = {
  backgroundColor: c.border,
  color: c.textPrimary,
  border: 'none',
  borderRadius: 8,
  padding: '6px 12px',
  fontWeight: 700,
  cursor: 'pointer',
};

export const ThemeSettingsPanel: React.FC = () => {
  const { t } = useI18n();
  const { preferences, theme, setPreferences, setCustomColor, resetPreferences } = useTheme();
  const titleId = useId();
  const a11yId = useId();
  const sizeId = useId();
  const [message, setMessage] = useState('');

  const onColor = (slot: ColorSlot, value: string | undefined) => {
    const label = t(`theme.slot.${slot}`);
    if (setCustomColor(slot, value)) {
      setMessage(value ? t('theme.colorApplied', { slot: label }) : t('theme.colorReset', { slot: label }));
    } else {
      setMessage(t('theme.colorRejected', { slot: label }));
    }
  };

  return (
    <section
      aria-labelledby={titleId}
      style={{
        backgroundColor: c.bgSurface,
        color: c.textPrimary,
        border: `1px solid ${c.border}`,
        borderRadius: 12,
        padding: 20,
        display: 'flex',
        flexDirection: 'column',
        gap: 14,
      }}
    >
      <h2 id={titleId} style={{ margin: 0, fontSize: '1.25rem' }}>
        {t('theme.title')}
      </h2>
      <p style={{ margin: 0, color: c.textMuted }}>
        {t('theme.active', { name: t(`theme.id.${theme.highContrast ? 'highContrast' : preferences.family}`) })}{' '}
        <span style={{ color: c.textSecondary }}>
          ({t(`theme.mode.${theme.scheme}`)})
        </span>
      </p>

      <fieldset style={fieldsetStyle}>
        <legend style={legendStyle}>{t('theme.family')}</legend>
        {THEME_FAMILIES.map((family) => {
          const swatch = PALETTES[family][theme.scheme];
          const selected = preferences.family === family;
          return (
            <label key={family} style={choiceStyle(selected)}>
              <input
                type="radio"
                name={`${titleId}-family`}
                value={family}
                checked={selected}
                onChange={() => setPreferences({ family })}
              />
              <span
                aria-hidden="true"
                style={{
                  display: 'inline-block',
                  width: 28,
                  height: 18,
                  borderRadius: 4,
                  border: `1px solid ${c.border}`,
                  background: `linear-gradient(90deg, ${swatch.bgSurface} 50%, ${swatch.accent} 50%)`,
                }}
              />
              {t(`theme.id.${family}`)}
            </label>
          );
        })}
      </fieldset>

      <fieldset style={fieldsetStyle}>
        <legend style={legendStyle}>{t('theme.modeLabel')}</legend>
        {THEME_MODES.map((mode: ThemeMode) => (
          <label key={mode} style={choiceStyle(preferences.mode === mode)}>
            <input
              type="radio"
              name={`${titleId}-mode`}
              value={mode}
              checked={preferences.mode === mode}
              onChange={() => setPreferences({ mode })}
            />
            {t(`theme.mode.${mode}`)}
          </label>
        ))}
      </fieldset>

      <div style={{ display: 'flex', flexWrap: 'wrap', gap: 16, alignItems: 'center' }}>
        <label htmlFor={a11yId} style={{ fontWeight: 700 }}>
          {t('theme.accessibility')}
        </label>
        <select
          id={a11yId}
          value={preferences.accessibility}
          onChange={(e) => setPreferences({ accessibility: e.target.value as AccessibilityProfile })}
          style={{
            backgroundColor: c.bgRaised,
            color: c.textPrimary,
            border: `1px solid ${c.textMuted}`,
            borderRadius: 6,
            padding: '4px 8px',
          }}
        >
          {ACCESSIBILITY_PROFILES.map((profile) => (
            <option key={profile} value={profile}>
              {t(`theme.profile.${profile}`)}
            </option>
          ))}
        </select>

        <label htmlFor={sizeId} style={{ fontWeight: 700 }}>
          {t('theme.textSize')}
        </label>
        <select
          id={sizeId}
          value={preferences.fontScale}
          onChange={(e) => setPreferences({ fontScale: Number(e.target.value) })}
          style={{
            backgroundColor: c.bgRaised,
            color: c.textPrimary,
            border: `1px solid ${c.textMuted}`,
            borderRadius: 6,
            padding: '4px 8px',
          }}
        >
          {FONT_SCALES.map((scale) => (
            <option key={scale} value={scale}>
              {scale}%
            </option>
          ))}
        </select>

        <label style={{ display: 'inline-flex', gap: 6, alignItems: 'center' }}>
          <input
            type="checkbox"
            checked={preferences.reducedMotion}
            onChange={(e) => setPreferences({ reducedMotion: e.target.checked })}
          />
          {t('theme.reducedMotion')}
        </label>
      </div>

      <fieldset style={{ ...fieldsetStyle, flexDirection: 'column' }}>
        <legend style={legendStyle}>{t('theme.customColors')}</legend>
        <p style={{ margin: 0, color: c.textMuted }}>
          {t('theme.customHint', { ratio: theme.minTextContrast })}
        </p>
        <ul style={{ listStyle: 'none', padding: 0, margin: 0, display: 'grid', gap: 8 }}>
          {COLOR_SLOT_NAMES.map((slot) => {
            const custom = preferences.customColors[slot];
            const rejected = theme.rejectedSlots.includes(slot);
            const inputId = `${titleId}-color-${slot}`;
            return (
              <li key={slot} style={{ display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
                <label htmlFor={inputId} style={{ minWidth: 110 }}>
                  {t(`theme.slot.${slot}`)}
                </label>
                <input
                  id={inputId}
                  type="color"
                  value={(theme.tokens[COLOR_SLOTS[slot]] ?? '#000000').toLowerCase()}
                  onChange={(e) => onColor(slot, e.target.value)}
                />
                {custom && (
                  <button
                    type="button"
                    style={buttonStyle}
                    aria-label={t('theme.resetSlot', { slot: t(`theme.slot.${slot}`) })}
                    onClick={() => onColor(slot, undefined)}
                  >
                    {t('theme.reset')}
                  </button>
                )}
                {rejected && (
                  <span style={{ color: c.warning }}>
                    <span aria-hidden="true">⚠ </span>
                    {t('theme.slotIgnored')}
                  </span>
                )}
              </li>
            );
          })}
        </ul>
      </fieldset>

      <p role="status" aria-live="polite" style={{ margin: 0, minHeight: '1.2em', color: c.textSecondary }}>
        {message}
      </p>

      <div>
        <button
          type="button"
          style={buttonStyle}
          onClick={() => {
            resetPreferences();
            setMessage(t('theme.resetDone'));
          }}
        >
          {t('theme.resetAll')}
        </button>
      </div>
    </section>
  );
};
