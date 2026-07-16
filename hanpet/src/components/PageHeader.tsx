interface Props {
  title: string;
  subtitle?: string;
  actions?: React.ReactNode;
}

export function PageHeader({ title, subtitle, actions }: Props) {
  return (
    <div className="page-header page-header--with-actions">
      <div className="page-header-text">
        <div className="page-header-title-row">
          <h1 className="page-title">{title}</h1>
          <span className="page-header-spark" aria-hidden>
            ✦
          </span>
        </div>
        {subtitle && <p className="page-subtitle">{subtitle}</p>}
      </div>
      {actions && <div className="page-header-actions">{actions}</div>}
    </div>
  );
}
