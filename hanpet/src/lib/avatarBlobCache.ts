import { xiaohan } from "./xiaohan";

const MAX_ENTRIES = 48;
const cache = new Map<string, string>();

function cacheKey(characterId: string, avatarPath: string): string {
  return `${characterId}\0${avatarPath}`;
}

function mimeFromPath(path: string): string {
  const lower = path.toLowerCase();
  if (lower.endsWith(".png")) return "image/png";
  if (lower.endsWith(".webp")) return "image/webp";
  return "image/jpeg";
}

function evictOldest() {
  const first = cache.keys().next().value;
  if (!first) return;
  const url = cache.get(first);
  if (url) URL.revokeObjectURL(url);
  cache.delete(first);
}

function blobUrlFromBase64(characterId: string, avatarPath: string, b64: string): string {
  const key = cacheKey(characterId, avatarPath);
  const hit = cache.get(key);
  if (hit) return hit;
  while (cache.size >= MAX_ENTRIES) evictOldest();
  const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
  const url = URL.createObjectURL(new Blob([bytes], { type: mimeFromPath(avatarPath) }));
  cache.set(key, url);
  return url;
}

/** 从本地缓存文件读取头像为 blob URL（IPC，不依赖 asset 协议） */
export async function loadAvatarBlobUrl(
  characterId: string,
  avatarPath: string
): Promise<string | null> {
  const key = cacheKey(characterId, avatarPath);
  const hit = cache.get(key);
  if (hit) return hit;
  try {
    const b64 = await xiaohan.charactersReadAvatar(characterId);
    if (!b64) return null;
    return blobUrlFromBase64(characterId, avatarPath, b64);
  } catch {
    return null;
  }
}

/** 移除某人物的全部 blob 缓存（路径更新或批量下载后调用） */
export function revokeAvatarBlob(characterId: string) {
  const prefix = `${characterId}\0`;
  for (const key of [...cache.keys()]) {
    if (!key.startsWith(prefix)) continue;
    const url = cache.get(key);
    if (url) URL.revokeObjectURL(url);
    cache.delete(key);
  }
}
