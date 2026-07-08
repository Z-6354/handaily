import { useEffect, useRef, type RefObject } from "react";

/** 滚动到底部时触发 loadMore */
export function useInfiniteScroll(
  enabled: boolean,
  onLoadMore: () => void,
  rootRef?: RefObject<HTMLElement | null>
) {
  const sentinelRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    if (!enabled) return;
    const el = sentinelRef.current;
    if (!el) return;

    const observer = new IntersectionObserver(
      (entries) => {
        if (entries.some((e) => e.isIntersecting)) {
          onLoadMore();
        }
      },
      {
        root: rootRef?.current ?? el.parentElement,
        rootMargin: "160px",
        threshold: 0,
      }
    );
    observer.observe(el);
    return () => observer.disconnect();
  }, [enabled, onLoadMore, rootRef]);

  return sentinelRef;
}
