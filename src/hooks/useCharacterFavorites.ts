import { useCallback, useEffect, useRef, useState } from "react";
import { xiaohan } from "../lib/xiaohan";

const FAVORITES_SETTING_KEY = "character_favorites";

function parseFavoriteIds(raw: string | null): string[] {
  if (!raw?.trim()) return [];
  try {
    const parsed = JSON.parse(raw) as unknown;
    if (!Array.isArray(parsed)) return [];
    return parsed.filter((id): id is string => typeof id === "string" && id.length > 0);
  } catch {
    return [];
  }
}

export function useCharacterFavorites() {
  const [favoriteIds, setFavoriteIds] = useState<string[]>([]);
  const [loaded, setLoaded] = useState(false);
  const favoriteIdsRef = useRef(favoriteIds);

  useEffect(() => {
    favoriteIdsRef.current = favoriteIds;
  }, [favoriteIds]);

  useEffect(() => {
    let cancelled = false;
    xiaohan
      .getSetting(FAVORITES_SETTING_KEY)
      .then((raw: string | null) => {
        if (!cancelled) {
          setFavoriteIds(parseFavoriteIds(raw));
          setLoaded(true);
        }
      })
      .catch(() => {
        if (!cancelled) setLoaded(true);
      });
    return () => {
      cancelled = true;
    };
  }, []);

  const toggleFavorite = useCallback(async (characterId: string) => {
    const current = favoriteIdsRef.current;
    const next = current.includes(characterId)
      ? current.filter((id) => id !== characterId)
      : [characterId, ...current];
    setFavoriteIds(next);
    try {
      await xiaohan.saveSetting(FAVORITES_SETTING_KEY, JSON.stringify(next));
    } catch {
      setFavoriteIds(current);
    }
  }, []);

  const isFavorite = useCallback(
    (characterId: string) => favoriteIds.includes(characterId),
    [favoriteIds]
  );

  return { favoriteIds, loaded, toggleFavorite, isFavorite };
}
