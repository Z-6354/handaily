import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { CharacterBrief } from "../lib/xiaohan";
import { revokeAvatarBlob } from "../lib/avatarBlobCache";
import { xiaohan } from "../lib/xiaohan";

export const GRID_COLS = 6;
export const GRID_ROWS = 3;
export const GRID_SLOTS = GRID_COLS * GRID_ROWS;
/** 默认页：17 人物 + 1 导入位 */
export const ROSTER_PAGE_WITH_ADD = GRID_SLOTS - 1;
/** 搜索 / 收藏页：满 18 格人物 */
export const ROSTER_PAGE_FULL = GRID_SLOTS;

function applyAvatarPaths(
  items: CharacterBrief[],
  paths: Record<string, string>
): CharacterBrief[] {
  if (!paths || Object.keys(paths).length === 0) return items;
  return items.map((c) =>
    paths[c.id] ? { ...c, avatar_path: paths[c.id] } : c
  );
}

async function fetchRosterPage(
  offset: number,
  limit: number,
  query: string,
  favoritesOnly: boolean,
  favoriteIds: string[],
  attempt = 0
) {
  const trimmed = query.trim();
  try {
    return await xiaohan.charactersListPage({
      offset,
      limit,
      query: trimmed || undefined,
      favoritesOnly: favoritesOnly || undefined,
      favoriteIds: favoritesOnly && favoriteIds.length > 0 ? favoriteIds : undefined,
    });
  } catch (e) {
    if (attempt < 1) {
      await new Promise((r) => window.setTimeout(r, 400));
      return fetchRosterPage(offset, limit, query, favoritesOnly, favoriteIds, attempt + 1);
    }
    throw e;
  }
}

/** 人物列表：服务端分页，3×6 固定网格；头像由启动时后台同步，页面只读本地缓存 */
export function useCharacterRoster(options: {
  query: string;
  page: number;
  pageSize: number;
  favoritesOnly: boolean;
  favoriteIds: string[];
  onAvatarsCached?: (paths: Record<string, string>) => void;
}) {
  const { query, page, pageSize, favoritesOnly, favoriteIds, onAvatarsCached } = options;
  const favoriteKey = favoritesOnly ? favoriteIds.join("\0") : "";
  const rosterFilterKey = useMemo(
    () => `${query}\0${favoritesOnly}\0${favoriteKey}\0${pageSize}`,
    [query, favoritesOnly, favoriteKey, pageSize]
  );
  const requestKey = `${rosterFilterKey}\0${page}`;

  const [characters, setCharacters] = useState<CharacterBrief[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestKeyRef = useRef(requestKey);
  const rosterFilterRef = useRef(rosterFilterKey);
  const hasDataRef = useRef(false);
  const onAvatarsCachedRef = useRef(onAvatarsCached);

  useEffect(() => {
    onAvatarsCachedRef.current = onAvatarsCached;
  }, [onAvatarsCached]);

  const totalPages = Math.max(1, Math.ceil(total / pageSize));

  const patchAvatarPaths = useCallback((paths: Record<string, string>) => {
    if (!paths || Object.keys(paths).length === 0) return;
    for (const id of Object.keys(paths)) revokeAvatarBlob(id);
    setCharacters((prev) => applyAvatarPaths(prev, paths));
    onAvatarsCachedRef.current?.(paths);
  }, []);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void listen<Record<string, string>>("avatars-cached", (ev) => {
      if (ev.payload) patchAvatarPaths(ev.payload);
    }).then((fn) => {
      unlisten = fn;
    });
    return () => {
      unlisten?.();
    };
  }, [patchAvatarPaths]);

  const loadCurrentPage = useCallback(async () => {
    const safePage = Math.max(1, page);
    const offset = (safePage - 1) * pageSize;
    requestKeyRef.current = requestKey;
    const pageTurn = rosterFilterRef.current === rosterFilterKey && hasDataRef.current;
    rosterFilterRef.current = rosterFilterKey;
    if (pageTurn) {
      setRefreshing(true);
    } else {
      hasDataRef.current = false;
      setCharacters([]);
      setLoading(true);
    }
    setError(null);
    try {
      const result = await fetchRosterPage(
        offset,
        pageSize,
        query,
        favoritesOnly,
        favoriteIds
      );
      if (requestKeyRef.current !== requestKey) return;
      setTotal(result.total);
      setCharacters(result.items);
      hasDataRef.current = result.items.length > 0;
      setLoading(false);
      setRefreshing(false);
    } catch (e) {
      if (requestKeyRef.current === requestKey) {
        setError(String(e));
        if (!pageTurn) {
          setCharacters([]);
          setTotal(0);
          hasDataRef.current = false;
        }
      }
    } finally {
      if (requestKeyRef.current === requestKey) {
        setLoading(false);
        setRefreshing(false);
      }
    }
  }, [requestKey, rosterFilterKey, query, favoritesOnly, favoriteIds, page, pageSize]);

  useEffect(() => {
    void loadCurrentPage();
  }, [loadCurrentPage]);

  return {
    characters,
    total,
    totalPages,
    loading,
    refreshing,
    error,
    refresh: loadCurrentPage,
  };
}
