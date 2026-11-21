import React, { useId, useState } from 'react';
import { colors } from '../../a11y';
import { useI18n } from '../../i18n';

export interface AddressInputProps {
  value: string;
  onChange: (val: string) => void;
  label?: string;
  placeholder?: string;
}

export const AddressInput: React.FC<AddressInputProps> = ({
  value,
  onChange,
  label,
  placeholder = 'G...',
}) => {
  const { t } = useI18n();
  const inputId = useId();
  const errorId = `${inputId}-error`;
  const [touched, setTouched] = useState(false);

  // Stellar G-address validation: 56 chars starting with G
  const isValidStellarAddress = (addr: string) => {
    if (!addr) return false;
    return addr.length === 56 && addr.startsWith('G');
  };

  const isInvalid = touched && value.length > 0 && !isValidStellarAddress(value);

  return (
    <div style={{ display: 'flex', flexDirection: 'column', gap: '4px', width: '100%' }}>
      <label htmlFor={inputId} style={{ fontSize: '0.8rem', color: colors.textMuted }}>
        {label ?? t('address.label')}
      </label>
      <input
        id={inputId}
        type="text"
        dir="ltr"
        autoComplete="off"
        spellCheck={false}
        value={value}
        onChange={(e) => onChange(e.target.value.trim())}
        onBlur={() => setTouched(true)}
        placeholder={placeholder}
        aria-invalid={isInvalid}
        aria-describedby={isInvalid ? errorId : undefined}
        style={{
          backgroundColor: colors.bgSurface,
          border: `1px solid ${isInvalid ? colors.danger : colors.textMuted}`,
          borderRadius: '6px',
          padding: '8px 12px',
          color: colors.textPrimary,
          fontSize: '0.85rem',
          fontFamily: 'monospace',
        }}
      />
      {isInvalid && (
        <span id={errorId} role="alert" style={{ color: colors.danger, fontSize: '0.75rem' }}>
          {t('address.invalid')}
        </span>
      )}
    </div>
  );
};
