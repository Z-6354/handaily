import { useCallback, useEffect, useState } from "react";
import { xiaohan } from "../lib/xiaohan";

const HISTORY_KEY = "persona_search_history";
const MAX_ITEMS = 12;

export function useSearchHistory() {
  const [history, setHistory] = useState<string[]>([]);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void (async () => {
      try {
        const raw = await xiaohan.getSetting(HISTORY_KEY);
        if (cancelled) return;
        if (raw) {
          const parsed = JSON.parse(raw) as unknown;
          if (Array.isArray(parsed)) {
            setHistory(
              parsed.filter((x): x is string => typeof x === "string").slice(0, MAX_ITEMS)
            );
          }
        }
      } catch {
        /* ignore */
      } finally {
        if (!cancelled) setReady(true);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, []);

  const persist = useCallback(async (items: string[]) => {
    setHistory(items);
    try {
      await xiaohan.saveSetting(HISTORY_KEY, JSON.stringify(items));
    } catch {
      /* ignore */
    }
  }, []);

  const add = useCallback(
    (query: string) => {
      const q = query.trim();
      if (!q) return;
      const next = [q, ...history.filter((h) => h !== q)].slice(0, MAX_ITEMS);
      void persist(next);
    },
    [history, persist]
  );

  const remove = useCallback(
    (query: string) => {
      void persist(history.filter((h) => h !== query));
    },
    [history, persist]
  );

  const clear = useCallback(() => {
    void persist([]);
  }, [persist]);

  return { history, ready, add, remove, clear };
}
