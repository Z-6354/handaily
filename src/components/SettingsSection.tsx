import type { ReactNode } from "react";

interface Props {
  title: string;
  description?: string;
  children: ReactNode;
}

export function SettingsSection({ title, description, children }: Props) {
  return (
    <section className="settings-section">
      <header className="settings-section-head">
        <h3 className="settings-section-title">{title}</h3>
        {description && <p className="settings-section-desc">{description}</p>}
      </header>
      <div className="settings-section-body">{children}</div>
    </section>
  );
}
