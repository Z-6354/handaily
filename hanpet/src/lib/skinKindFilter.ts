import type { CharacterSkinInfo } from "./xiaohan";

/** Peer presentation kinds for one skin slot (Phase 1 UI projection). */
export type SkinKind = "spine" | "kanmusu";

export const SKIN_KIND_STORAGE_KEY = "hanpet.skinKindTab";

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

export function readStoredSkinKind(): SkinKind {
  try {
    const v = sessionStorage.getItem(SKIN_KIND_STORAGE_KEY);
    if (v === "kanmusu" || v === "spine") return v;
  } catch {
    /* ignore */
  }
  return "spine";
}

export function writeStoredSkinKind(kind: SkinKind): void {
  try {
    sessionStorage.setItem(SKIN_KIND_STORAGE_KEY, kind);
  } catch {
    /* ignore */
  }
}
