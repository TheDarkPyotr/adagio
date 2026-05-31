import React, { useState, useEffect } from 'react';
import { Icon, FileGlyph, StatusDot } from './shared';
import type { FileStatusDto } from '../tauri';
import { listSyncedFiles } from '../tauri';

export type FilterSection = 'fav' | 'recent' | 'shared' | 'tags';

const TITLES: Record<FilterSection, string> = {
  fav: 'Favorites', recent: 'Recent', shared: 'Shared', tags: 'Tagged',
};

const EMPTY: Record<FilterSection, string> = {
  fav: 'No favorites yet — star files while browsing to collect them here.',
  recent: 'No files found.',
  shared: 'No shared files.',
  tags: 'No tagged files — add tags while browsing All files.',
};

function fmtBytes(n: number | null): string {
  if (n === null) return '—';
  if (n >= 1e9) return (n / 1e9).toFixed(1) + ' GB';
  if (n >= 1e6) return (n / 1e6).toFixed(1) + ' MB';
  if (n >= 1e3) return (n / 1e3).toFixed(0) + ' KB';
  return n + ' B';
}

function fmtAgo(ts: number | null): string {
  if (!ts) return '—';
  const diff = Date.now() - ts;
  const mins = Math.floor(diff / 60000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins}m ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  const days = Math.floor(hrs / 24);
  if (days < 7) return `${days}d ago`;
  return new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric' }).format(new Date(ts));
}

function tagColor(tag: string): string {
  const palette = ['#c8542a', '#2f6fcf', '#1c8c6e', '#9550b8', '#b76b35', '#c89020'];
  let h = 0;
  for (let i = 0; i < tag.length; i++) h = (h * 31 + tag.charCodeAt(i)) & 0xffff;
  return palette[h % palette.length];
}

export default function SectionScene({
  section, pairId,
  favorites, allTaggedFiles, pathTags,
  onToggleFavorite, onAddTag, onRemoveTag, onShare, onOpenFolder, onSharedCount,
}: {
  section: FilterSection;
  pairId: string | null;
  favorites: Map<string, FileStatusDto>;
  allTaggedFiles: Map<string, FileStatusDto>;
  pathTags: Record<string, string[]>;
  onToggleFavorite: (file: FileStatusDto) => void;
  onAddTag: (file: FileStatusDto, tag: string) => void;
  onRemoveTag: (path: string, tag: string) => void;
  onShare: (path: string) => void;
  onOpenFolder: (path: string) => void;
  onSharedCount?: (count: number) => void;
}) {
  // flat file list built by BFS traversal — used for 'recent' and 'shared'
  const [allFiles, setAllFiles] = useState<FileStatusDto[]>([]);
  const [loading, setLoading] = useState(false);
  const [tagFilter, setTagFilter] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);

  const needsBackend = section === 'recent' || section === 'shared';

  useEffect(() => {
    if (!needsBackend || !pairId) return;
    let cancelled = false;
    setLoading(true);
    setAllFiles([]);

    // BFS across the entire tree; stops early if cancelled
    const crawl = async () => {
      const result: FileStatusDto[] = [];
      const queue = ['/'];
      while (queue.length > 0) {
        if (cancelled) return;
        const dir = queue.shift()!;
        try {
          const items = await listSyncedFiles(pairId, dir);
          for (const item of items) {
            result.push(item);
            if (item.is_dir) queue.push(item.path);
          }
        } catch {}
      }
      if (!cancelled) setAllFiles(result);
    };
    crawl().finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [pairId, needsBackend]);

  let displayed: FileStatusDto[];

  if (section === 'fav') {
    displayed = Array.from(favorites.values());
  } else if (section === 'recent') {
    displayed = [...allFiles]
      .filter(f => f.mtime != null && !f.is_dir)
      .sort((a, b) => (b.mtime ?? 0) - (a.mtime ?? 0))
      .slice(0, 100);
  } else if (section === 'shared') {
    displayed = allFiles.filter(f => (f.share_count ?? 0) > 0);
  } else {
    // tags — use stored file snapshots, no backend call
    const all = Array.from(allTaggedFiles.values());
    displayed = tagFilter ? all.filter(f => pathTags[f.path]?.includes(tagFilter)) : all;
  }

  // Notify parent of shared count once the BFS completes so sidebar badge is accurate.
  const sharedCount = section === 'shared' ? displayed.length : 0;
  React.useEffect(() => {
    if (section === 'shared') onSharedCount?.(sharedCount);
  }, [section, sharedCount, onSharedCount]);

  // Tag sidebar data
  const allTags = section === 'tags'
    ? [...new Set(Array.from(allTaggedFiles.keys()).flatMap(p => pathTags[p] ?? []))]
    : [];

  const col4Label = section === 'recent' ? 'Modified'
    : section === 'shared' ? 'Shared'
    : section === 'tags' ? 'Tags'
    : 'Path';

  return (
    <div style={{ flex: 1, display: 'flex', overflow: 'hidden', background: 'var(--cream)' }}>
      {/* tag sidebar */}
      {section === 'tags' && allTags.length > 0 && (
        <div style={{ width: 160, flexShrink: 0, borderRight: '1px solid var(--hairline)', background: 'var(--paper)', display: 'flex', flexDirection: 'column', padding: '14px 8px' }}>
          <div style={{ padding: '2px 8px 10px', fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.16em', textTransform: 'uppercase', color: 'var(--ink-muted)' }}>Tags</div>
          <TagPill label="All" color="var(--ink-muted)" active={tagFilter === null}
            onClick={() => setTagFilter(null)} count={allTaggedFiles.size} />
          {allTags.map(tag => (
            <TagPill key={tag} label={tag} color={tagColor(tag)} active={tagFilter === tag}
              onClick={() => setTagFilter(t => t === tag ? null : tag)}
              count={Array.from(allTaggedFiles.keys()).filter(p => pathTags[p]?.includes(tag)).length} />
          ))}
        </div>
      )}

      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
        {/* header */}
        <div style={{ height: 52, flexShrink: 0, padding: '0 22px', display: 'flex', alignItems: 'center', borderBottom: '1px solid var(--hairline)' }}>
          <span style={{ fontWeight: 600, fontSize: 14, letterSpacing: '-0.02em' }}>
            {tagFilter ?? TITLES[section]}
          </span>
          {displayed.length > 0 && (
            <span style={{ fontFamily: 'var(--mono)', fontWeight: 400, fontSize: 11, color: 'var(--ink-muted)', marginLeft: 8 }}>
              {displayed.length}
            </span>
          )}
        </div>

        {/* column headers */}
        <div style={{ display: 'grid', gridTemplateColumns: '38px 1fr 100px 160px 80px', padding: '10px 22px', fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.14em', textTransform: 'uppercase', color: 'var(--ink-muted)', borderBottom: '1px solid var(--hairline)', flexShrink: 0 }}>
          <span /><span>Name</span><span>Size</span><span>{col4Label}</span>
          <span style={{ textAlign: 'right' }}>Status</span>
        </div>

        {/* rows */}
        <div style={{ flex: 1, overflowY: 'auto', padding: '4px 0 12px' }}>
          {loading && <p style={{ padding: '24px 22px', textAlign: 'center', color: 'var(--ink-muted)', fontSize: 13 }}>Loading…</p>}
          {!loading && displayed.length === 0 && (
            <p style={{ padding: '24px 22px', textAlign: 'center', color: 'var(--ink-muted)', fontSize: 13 }}>{EMPTY[section]}</p>
          )}
          {displayed.map(f => (
            <SectionRow
              key={f.path}
              file={f}
              section={section}
              selected={selected === f.path}
              starred={favorites.has(f.path)}
              fileTags={pathTags[f.path] ?? []}
              onSelect={() => setSelected(p => p === f.path ? null : f.path)}
              onOpenFolder={onOpenFolder}
              onToggleStar={() => onToggleFavorite(f)}
              onAddTag={tag => onAddTag(f, tag)}
              onRemoveTag={tag => onRemoveTag(f.path, tag)}
              onShare={() => onShare(f.path)}
            />
          ))}
        </div>

        {/* status bar */}
        <div style={{ height: 32, flexShrink: 0, borderTop: '1px solid var(--hairline)', background: 'var(--paper)', display: 'flex', alignItems: 'center', gap: 12, padding: '0 20px', fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)', letterSpacing: '0.04em' }}>
          <span>{displayed.length} item{displayed.length !== 1 ? 's' : ''}</span>
          {selected && <span>· 1 selected</span>}
          {selected && displayed.find(f => f.path === selected)?.is_dir && (
            <span>· double-click to open folder</span>
          )}
        </div>
      </div>
    </div>
  );
}

function TagPill({ label, color, active, onClick, count }: {
  label: string; color: string; active: boolean; onClick: () => void; count: number;
}) {
  const [h, setH] = useState(false);
  return (
    <button onClick={onClick} onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '6px 8px', borderRadius: 'var(--r-1)', background: active ? 'var(--cream-2)' : h ? 'color-mix(in srgb, var(--ink) 4%, transparent)' : 'transparent', border: 'none', cursor: 'pointer', width: '100%', textAlign: 'left', transition: 'background 0.08s' }}>
      <div style={{ width: 8, height: 8, borderRadius: 4, background: color, flexShrink: 0 }} />
      <span style={{ flex: 1, fontSize: 12.5, color: active ? 'var(--ink)' : 'var(--ink-soft)', fontWeight: active ? 500 : 400, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{label}</span>
      <span style={{ fontFamily: 'var(--mono)', fontSize: 10.5, color: 'var(--ink-muted)' }}>{count}</span>
    </button>
  );
}

function SectionRow({ file, section, selected, starred, fileTags, onSelect, onOpenFolder, onToggleStar, onAddTag, onRemoveTag, onShare }: {
  file: FileStatusDto;
  section: FilterSection;
  selected: boolean;
  starred: boolean;
  fileTags: string[];
  onSelect: () => void;
  onOpenFolder: (path: string) => void;
  onToggleStar: () => void;
  onAddTag: (tag: string) => void;
  onRemoveTag: (tag: string) => void;
  onShare: () => void;
}) {
  const [hovered, setHovered] = useState(false);
  const [tagInput, setTagInput] = useState('');
  const [tagEditing, setTagEditing] = useState(false);
  const kind = file.is_dir ? 'folder' : (file.name.split('.').pop()?.toLowerCase() ?? 'file');

  const col4 = (() => {
    if (section === 'recent') {
      return <span style={{ fontSize: 12, color: 'var(--ink-muted)' }}>{fmtAgo(file.mtime)}</span>;
    }
    if (section === 'shared') {
      return (
        <span style={{ display: 'flex', alignItems: 'center', gap: 4 }}>
          <Icon name="people" size={12} color="var(--ink-muted)" />
          <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)' }}>{file.share_count ?? 0}</span>
        </span>
      );
    }
    if (section === 'tags') {
      return (
        <div style={{ display: 'flex', alignItems: 'center', gap: 4, flexWrap: 'wrap' }}>
          {fileTags.map(tag => (
            <span key={tag} style={{ display: 'inline-flex', alignItems: 'center', gap: 2, background: tagColor(tag) + '1e', color: tagColor(tag), borderRadius: 4, padding: '1px 6px', fontSize: 10.5, fontFamily: 'var(--mono)', whiteSpace: 'nowrap' }}>
              {tag}
              <button onClick={e => { e.stopPropagation(); onRemoveTag(tag); }}
                style={{ background: 'none', border: 'none', cursor: 'pointer', padding: '0 0 0 2px', color: 'inherit', lineHeight: 1, fontSize: 12 }}>×</button>
            </span>
          ))}
          {tagEditing ? (
            <input autoFocus value={tagInput} onChange={e => setTagInput(e.target.value)}
              onKeyDown={e => {
                if (e.key === 'Enter' && tagInput.trim()) { onAddTag(tagInput.trim()); setTagInput(''); setTagEditing(false); }
                if (e.key === 'Escape') { setTagInput(''); setTagEditing(false); }
              }}
              onBlur={() => { if (!tagInput.trim()) setTagEditing(false); }}
              style={{ border: '1px solid var(--hairline)', borderRadius: 4, padding: '1px 5px', fontSize: 10.5, fontFamily: 'var(--mono)', background: 'var(--cream)', color: 'var(--ink)', width: 70, outline: 'none' }}
              placeholder="tag…" />
          ) : hovered && (
            <button onClick={e => { e.stopPropagation(); setTagEditing(true); }}
              style={{ background: 'none', border: '1px dashed var(--hairline)', borderRadius: 4, cursor: 'pointer', padding: '1px 5px', fontSize: 10.5, fontFamily: 'var(--mono)', color: 'var(--ink-muted)' }}>
              +tag
            </button>
          )}
        </div>
      );
    }
    // fav: show parent path
    const parent = file.path.split('/').slice(0, -1).join('/') || '/';
    return <span style={{ fontSize: 11, fontFamily: 'var(--mono)', color: 'var(--ink-muted)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{parent}</span>;
  })();

  return (
    <div
      onClick={onSelect}
      onDoubleClick={() => { if (file.is_dir) onOpenFolder(file.path); }}
      onMouseEnter={() => setHovered(true)} onMouseLeave={() => setHovered(false)}
      style={{ display: 'grid', gridTemplateColumns: '38px 1fr 100px 160px 80px', padding: '10px 22px', alignItems: 'center', background: selected ? 'var(--paper-2)' : hovered ? 'color-mix(in srgb, var(--ink) 4%, transparent)' : 'transparent', boxShadow: selected ? 'inset 2px 0 0 var(--clay)' : 'none', cursor: 'pointer', fontSize: 13.5 }}>
      <FileGlyph kind={kind} />
      <div style={{ display: 'flex', alignItems: 'center', gap: 8, minWidth: 0 }}>
        <span style={{ color: 'var(--ink)', fontWeight: file.is_dir ? 500 : 400, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{file.name}</span>
      </div>
      <span style={{ fontSize: 12, fontFamily: 'var(--mono)', color: 'var(--ink-muted)' }}>{fmtBytes(file.size)}</span>
      {col4}
      <span style={{ display: 'flex', justifyContent: 'flex-end', alignItems: 'center', gap: 5 }}>
        <StatusDot status={file.status} />
        {hovered && (
          <>
            <button onClick={onToggleStar} title={starred ? 'Unstar' : 'Star'}
              style={{ background: 'none', border: 'none', cursor: 'pointer', padding: 3, display: 'flex', borderRadius: 4, color: starred ? 'var(--clay)' : 'var(--ink-muted)' }}>
              <Icon name="star" size={13} color={starred ? 'var(--clay)' : 'var(--ink-muted)'} strokeWidth={starred ? 2 : 1.6} />
            </button>
            <button onClick={e => { e.stopPropagation(); onShare(); }} title="Share"
              style={{ background: 'none', border: 'none', cursor: 'pointer', padding: 3, display: 'flex', borderRadius: 4 }}>
              <Icon name="share" size={13} color="var(--ink-muted)" />
            </button>
          </>
        )}
      </span>
    </div>
  );
}
