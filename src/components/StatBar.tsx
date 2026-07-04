interface Stat {
  value: string | number;
  label: string;
}

interface Props {
  stats: Stat[];
  trailing?: React.ReactNode;
}

export function StatBar({ stats, trailing }: Props) {
  return (
    <div className="stat-bar">
      {stats.map((s, i) => (
        <div className="stat-bar-item" key={i}>
          <div className="stat-bar-value">{s.value}</div>
          <div className="stat-bar-label">{s.label}</div>
        </div>
      ))}
      {trailing && <div className="stat-bar-trailing">{trailing}</div>}
    </div>
  );
}
