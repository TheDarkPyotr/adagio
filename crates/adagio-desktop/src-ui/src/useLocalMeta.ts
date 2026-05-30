import { useState, useCallback, useMemo } from 'react';
import type { FileStatusDto } from './tauri';

function load<T>(key: string, fallback: T): T {
  try { return JSON.parse(localStorage.getItem(key) ?? 'null') ?? fallback; } catch { return fallback; }
}

export default function useLocalMeta() {
  // favorites stored as path→FileStatusDto so we can display any file regardless of depth
  const [favsObj, setFavsObj] = useState<Record<string, FileStatusDto>>(
    () => load('adagio:favorites', {}),
  );

  // tags: tag→paths (lightweight), file snapshots kept in a separate cache
  const [tags, setTags] = useState<Record<string, string[]>>(
    () => load('adagio:tags', {}),
  );
  const [tagCache, setTagCache] = useState<Record<string, FileStatusDto>>(
    () => load('adagio:tag-cache', {}),
  );

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
      localStorage.setItem('adagio:favorites', JSON.stringify(next));
      return next;
    });
  }, []);

  const addTag = useCallback((file: FileStatusDto, tag: string) => {
    setTags(prev => {
      const paths = prev[tag] ?? [];
      if (paths.includes(file.path)) return prev;
      const next = { ...prev, [tag]: [...paths, file.path] };
      localStorage.setItem('adagio:tags', JSON.stringify(next));
      return next;
    });
    setTagCache(prev => {
      if (prev[file.path]) return prev;
      const next = { ...prev, [file.path]: file };
      localStorage.setItem('adagio:tag-cache', JSON.stringify(next));
      return next;
    });
  }, []);

  const removeTag = useCallback((path: string, tag: string) => {
    setTags(prev => {
      const paths = (prev[tag] ?? []).filter(p => p !== path);
      const next = { ...prev };
      if (paths.length) next[tag] = paths; else delete next[tag];
      localStorage.setItem('adagio:tags', JSON.stringify(next));
      return next;
    });
  }, []);

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
