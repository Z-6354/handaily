import type { CharacterSkinInfo } from "./xiaohan";

/** Peer presentation kinds for one skin slot (Phase 1 UI projection). */
export type SkinKind = "spine" | "kanmusu";

export const SKIN_KIND_STORAGE_KEY = "hanpet.skinKindTab";

function charKindKey(characterId: string): string {
  return `hanpet.skinKind.${characterId}`;
}

export function skinMatchesKind(skin: CharacterSkinInfo, kind: SkinKind): boolean {
  if (kind === "spine") {
    return Boolean(skin.model_id?.trim()) || Boolean(skin.model_ready);
  }
  const dir = skin.kanmusu_dir?.trim();
  return Boolean(dir) || Boolean(skin.kanmusu_ready);
}

export function filterSkinsByKind(
  skins: CharacterSkinInfo[],
  kind: SkinKind,
): CharacterSkinInfo[] {
  return skins.filter((s) => skinMatchesKind(s, kind));
}

export function parseSkinKind(raw: string | null | undefined): SkinKind | null {
  if (raw === "kanmusu" || raw === "spine") return raw;
  return null;
}

export function readStoredSkinKind(): SkinKind {
  try {
    return parseSkinKind(sessionStorage.getItem(SKIN_KIND_STORAGE_KEY)) ?? "spine";
  } catch {
    return "spine";
  }
}

export function writeStoredSkinKind(kind: SkinKind): void {
  try {
    sessionStorage.setItem(SKIN_KIND_STORAGE_KEY, kind);
  } catch {
    /* ignore */
  }
}

/** Phase 3: per-character kind preference (UI tab); survives reloads via localStorage. */
export function readCharacterSkinKind(characterId: string): SkinKind | null {
  if (!characterId) return null;
  try {
    return parseSkinKind(localStorage.getItem(charKindKey(characterId)));
  } catch {
    return null;
  }
}

export function writeCharacterSkinKind(characterId: string, kind: SkinKind): void {
  if (!characterId) return;
  try {
    localStorage.setItem(charKindKey(characterId), kind);
  } catch {
    /* ignore */
  }
  writeStoredSkinKind(kind);
}
