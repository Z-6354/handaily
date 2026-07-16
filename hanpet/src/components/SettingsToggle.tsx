interface Props {
  label: string;
  hint?: string;
  checked: boolean;
  onChange: (next: boolean) => void;
  disabled?: boolean;
}

export function SettingsToggle({ label, hint, checked, onChange, disabled }: Props) {
  return (
    <div className="settings-field">
      <div className="settings-field-body">
        <div className="settings-field-label">{label}</div>
        {hint && <div className="settings-field-hint">{hint}</div>}
      </div>
      <button
        type="button"
        className={`toggle-switch${checked ? " on" : ""}`}
        aria-pressed={checked}
        disabled={disabled}
        onClick={() => onChange(!checked)}
      >
        <span className="toggle-thumb" />
      </button>
    </div>
  );
}
