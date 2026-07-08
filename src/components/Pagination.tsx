type Props = {
  page: number;
  totalPages: number;
  disabled?: boolean;
  onPageChange: (page: number) => void;
  className?: string;
};

function pageItems(current: number, total: number): (number | "ellipsis")[] {
  if (total <= 7) {
    return Array.from({ length: total }, (_, i) => i + 1);
  }
  const items: (number | "ellipsis")[] = [1];
  const left = Math.max(2, current - 1);
  const right = Math.min(total - 1, current + 1);
  if (left > 2) items.push("ellipsis");
  for (let p = left; p <= right; p += 1) items.push(p);
  if (right < total - 1) items.push("ellipsis");
  items.push(total);
  return items;
}

export function Pagination({
  page,
  totalPages,
  disabled,
  onPageChange,
  className = "",
}: Props) {
  if (totalPages <= 1) return null;

  const items = pageItems(page, totalPages);

  return (
    <nav
      className={`ui-pagination ${className}`.trim()}
      aria-label="分页"
    >
      <button
        type="button"
        className="ui-pagination-btn"
        disabled={disabled || page <= 1}
        onClick={() => onPageChange(page - 1)}
        aria-label="上一页"
      >
        ‹
      </button>
      {items.map((item, i) =>
        item === "ellipsis" ? (
          <span key={`e-${i}`} className="ui-pagination-ellipsis" aria-hidden>
            …
          </span>
        ) : (
          <button
            key={item}
            type="button"
            className={`ui-pagination-btn${item === page ? " is-active" : ""}`}
            disabled={disabled || item === page}
            aria-current={item === page ? "page" : undefined}
            onClick={() => onPageChange(item)}
          >
            {item}
          </button>
        )
      )}
      <button
        type="button"
        className="ui-pagination-btn"
        disabled={disabled || page >= totalPages}
        onClick={() => onPageChange(page + 1)}
        aria-label="下一页"
      >
        ›
      </button>
    </nav>
  );
}
