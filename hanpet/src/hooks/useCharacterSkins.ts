import { useCallback, useEffect, useRef, useState } from "react";
import type { CharacterSkinInfo } from "../lib/xiaohan";
import { xiaohan } from "../lib/xiaohan";

export const SKIN_PAGE_SIZE = 12;

export function useCharacterSkins(characterId: string | null, page: number) {
  const [skins, setSkins] = useState<CharacterSkinInfo[]>([]);
  const [total, setTotal] = useState(0);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef(0);

  const totalPages = Math.max(1, Math.ceil(total / SKIN_PAGE_SIZE));

  const fetchPage = useCallback(
    async (pageNum: number) => {
      if (!characterId) return;
      const requestId = ++requestRef.current;
      setLoading(true);
      setError(null);
      try {
        const offset = (Math.max(1, pageNum) - 1) * SKIN_PAGE_SIZE;
        const result = await xiaohan.charactersSkinsPage(
          characterId,
          offset,
          SKIN_PAGE_SIZE
        );
        if (requestRef.current !== requestId) return;
        setTotal(result.total);
        setSkins(result.items);
      } catch (e) {
        if (requestRef.current === requestId) {
          setError(e instanceof Error ? e.message : String(e));
          setSkins([]);
          setTotal(0);
        }
      } finally {
        if (requestRef.current === requestId) setLoading(false);
      }
    },
    [characterId]
  );

  const refresh = useCallback(async () => {
    await fetchPage(page);
  }, [fetchPage, page]);

  useEffect(() => {
    if (characterId) void fetchPage(page);
    else {
      ++requestRef.current;
      setSkins([]);
      setTotal(0);
      setLoading(false);
      setError(null);
    }
  }, [characterId, page, fetchPage]);

  return {
    skins,
    total,
    totalPages,
    loading,
    error,
    refresh,
  };
}
