interface Props {
  message: string;
  hint?: string;
}

export function EmptyState({ message, hint }: Props) {
  return (
    <div className="empty-state">
      <div className="empty-state-icon">📋</div>
      <p className="empty-state-msg">{message}</p>
      {hint && <p className="empty-state-hint">{hint}</p>}
    </div>
  );
}
