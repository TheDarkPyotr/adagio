import { useState, useCallback, useMemo, useEffect } from 'react';
import type { FileStatusDto } from './tauri';

function load<T>(key: string, fallback: T): T {
  try { return JSON.parse(localStorage.getItem(key) ?? 'null') ?? fallback; } catch { return fallback; }
}

// Scoped by pairId so each sync pair has independent favorites/tags.
export default function useLocalMeta(pairId: string | null) {
  const scope = pairId ?? 'default';
  const favKey      = `adagio:favorites:${scope}`;
  const tagsKey     = `adagio:tags:${scope}`;
  const tagCacheKey = `adagio:tag-cache:${scope}`;

  const [favsObj, setFavsObj] = useState<Record<string, FileStatusDto>>(() => load(favKey, {}));
  const [tags, setTags]       = useState<Record<string, string[]>>(() => load(tagsKey, {}));
  const [tagCache, setTagCache] = useState<Record<string, FileStatusDto>>(() => load(tagCacheKey, {}));

  // Reload all three stores whenever the active pair changes.
  useEffect(() => {
    setFavsObj(load(favKey, {}));
    setTags(load(tagsKey, {}));
    setTagCache(load(tagCacheKey, {}));
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scope]);

  // Filter out any corrupted entries (strings stored instead of FileStatusDto objects
  // due to a prior bug where the path string was passed instead of the full DTO).
  const favorites = useMemo(() => {
    const entries = Object.entries(favsObj).filter(
      ([, v]) => v !== null && typeof v === 'object' && typeof (v as FileStatusDto).name === 'string',
    ) as [string, FileStatusDto][];
    return new Map(entries);
  }, [favsObj]);

  const toggleFavorite = useCallback((file: FileStatusDto) => {
    setFavsObj(prev => {
      const next = { ...prev };
      if (next[file.path]) delete next[file.path];
      else next[file.path] = file;
      localStorage.setItem(favKey, JSON.stringify(next));
      return next;
    });
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [favKey]);

  const addTag = useCallback((file: FileStatusDto, tag: string) => {
    setTags(prev => {
      const paths = prev[tag] ?? [];
      if (paths.includes(file.path)) return prev;
      const next = { ...prev, [tag]: [...paths, file.path] };
      localStorage.setItem(tagsKey, JSON.stringify(next));
      return next;
    });
    setTagCache(prev => {
      if (prev[file.path]) return prev;
      const next = { ...prev, [file.path]: file };
      localStorage.setItem(tagCacheKey, JSON.stringify(next));
      return next;
    });
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tagsKey, tagCacheKey]);

  const removeTag = useCallback((path: string, tag: string) => {
    setTags(prev => {
      const paths = (prev[tag] ?? []).filter(p => p !== path);
      const next = { ...prev };
      if (paths.length) next[tag] = paths; else delete next[tag];
      localStorage.setItem(tagsKey, JSON.stringify(next));
      return next;
    });
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [tagsKey]);

  // path → [tag, …]
  const pathTags = useMemo(() => {
    const m: Record<string, string[]> = {};
    for (const [tag, paths] of Object.entries(tags))
      for (const p of paths) m[p] = [...(m[p] ?? []), tag];
    return m;
  }, [tags]);

  // all uniquely tagged files (for the Tags section)
  const allTaggedFiles = useMemo(() => {
    const m = new Map<string, FileStatusDto>();
    for (const paths of Object.values(tags))
      for (const p of paths) {
        const entry = tagCache[p];
        if (entry && typeof entry === 'object' && typeof (entry as FileStatusDto).name === 'string')
          m.set(p, entry as FileStatusDto);
      }
    return m;
  }, [tags, tagCache]);

  return { favorites, tags, pathTags, allTaggedFiles, toggleFavorite, addTag, removeTag };
}
