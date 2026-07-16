import { useCallback, useEffect, useRef, useState } from "react";

const DEFAULT_PAGE_SIZE = 48;

/** 人物网格懒加载：先展示一页，滚到底部自动加载更多 */
export function useRosterPagination<T>(filteredItems: T[], pageSize = DEFAULT_PAGE_SIZE) {
  const [visibleCount, setVisibleCount] = useState(pageSize);
  const sentinelRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setVisibleCount(pageSize);
  }, [filteredItems, pageSize]);

  const visibleItems = filteredItems.slice(0, visibleCount);
  const hasMore = visibleCount < filteredItems.length;

  const loadMore = useCallback(() => {
    setVisibleCount((n) => Math.min(n + pageSize, filteredItems.length));
  }, [filteredItems.length, pageSize]);

  useEffect(() => {
    const el = sentinelRef.current;
    if (!el || !hasMore) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          loadMore();
        }
      },
      { root: el.parentElement, rootMargin: "120px", threshold: 0 }
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [hasMore, loadMore, visibleItems.length]);

  return { visibleItems, hasMore, loadMore, sentinelRef, total: filteredItems.length };
}
