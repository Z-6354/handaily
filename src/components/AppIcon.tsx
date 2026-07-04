interface Props {
  icon?: string | null;
  name: string;
}

export function AppIcon({ icon, name }: Props) {
  const letter = (name.trim()[0] || "?").toUpperCase();
  if (icon) {
    return <img className="app-icon" src={icon} alt="" />;
  }
  return <span className="app-icon app-icon--fallback">{letter}</span>;
}
