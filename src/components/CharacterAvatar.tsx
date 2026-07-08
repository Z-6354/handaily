import { useEffect, useRef, useState } from "react";
import { characterInitial } from "../lib/characterDisplay";
import { loadAvatarBlobUrl } from "../lib/avatarBlobCache";
import { xiaohan } from "../lib/xiaohan";

type Props = {
  name: string;
  characterId?: string;
  avatarPath?: string | null;
  className?: string;
  /** 列表页仅展示本地缓存；启动时后台已批量下载 */
  deferDownload?: boolean;
  onCached?: (path: string) => void;
};

/** 头像展示本地缓存文件；有路径时经 IPC 读为 blob，无缓存时显示首字 */
export function CharacterAvatar({
  name,
  characterId,
  avatarPath,
  className = "",
  deferDownload = false,
  onCached,
}: Props) {
  const [blobSrc, setBlobSrc] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const downloadAttemptedRef = useRef<string | null>(null);

  useEffect(() => {
    setBlobSrc(null);
    setFailed(false);
    downloadAttemptedRef.current = null;
    if (!characterId || !avatarPath) return;

    let cancelled = false;
    void loadAvatarBlobUrl(characterId, avatarPath).then((url) => {
      if (cancelled) return;
      if (url) {
        setBlobSrc(url);
      } else {
        setFailed(true);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [avatarPath, characterId]);

  useEffect(() => {
    if (deferDownload || avatarPath || !characterId) return;
    if (downloadAttemptedRef.current === characterId) return;
    downloadAttemptedRef.current = characterId;
    let cancelled = false;
    void xiaohan
      .charactersCacheAvatar(characterId)
      .then((path) => {
        if (cancelled || !path) return;
        onCached?.(path);
        return loadAvatarBlobUrl(characterId, path);
      })
      .then((url) => {
        if (cancelled) return;
        if (url) setBlobSrc(url);
        else if (!cancelled) setFailed(true);
      })
      .catch(() => {
        if (!cancelled) setFailed(true);
      });
    return () => {
      cancelled = true;
    };
  }, [characterId, deferDownload, avatarPath, onCached]);

  if (blobSrc && !failed) {
    return (
      <img
        src={blobSrc}
        alt=""
        className={`persona-avatar-img ${className}`.trim()}
        loading="lazy"
        decoding="async"
        onError={() => setFailed(true)}
      />
    );
  }

  return (
    <span className={className || undefined} aria-hidden>
      {characterInitial(name)}
    </span>
  );
}
