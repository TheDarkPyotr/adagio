import React, { useState, useEffect, useCallback, useRef } from 'react';
import { createPortal } from 'react-dom';
import { open as shellOpen } from '@tauri-apps/plugin-shell';
import { Icon, FileGlyph, StatusDot, FlatBtn } from './shared';
import type { FileStatusDto, SyncStatusDto } from '../tauri';
import { listSyncedFiles } from '../tauri';

function fmtBytes(n: number | null): string {
  if (n === null) return '—';
  if (n >= 1e9) return (n / 1e9).toFixed(1) + ' GB';
  if (n >= 1e6) return (n / 1e6).toFixed(1) + ' MB';
  if (n >= 1e3) return (n / 1e3).toFixed(0) + ' KB';
  return n + ' B';
}

function fmtDate(ts: number | null): string {
  if (!ts) return '';
  return new Intl.DateTimeFormat(undefined, { month: 'short', day: 'numeric', year: 'numeric' }).format(new Date(ts));
}

interface ContextMenuState {
  x: number;
  y: number;
  file: FileStatusDto & { kind: string };
}

export default function FilesScene({ pairId, serverHost, currentPath, onPathChange, onShare, onNew, syncStatus, favorites, onToggleFavorite }: {
  pairId: string | null;
  serverHost: string;
  currentPath: string;
  onPathChange: (path: string) => void;
  onShare: (path: string) => void;
  onNew?: () => void;
  syncStatus?: SyncStatusDto | null;
  favorites?: Map<string, FileStatusDto>;
  onToggleFavorite?: (file: FileStatusDto) => void;
}) {
  const [files, setFiles] = useState<FileStatusDto[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [ctxMenu, setCtxMenu] = useState<ContextMenuState | null>(null);

  useEffect(() => { setSelected(null); setCtxMenu(null); }, [currentPath]);

  // Dismiss context menu on click outside
  useEffect(() => {
    if (!ctxMenu) return;
    const dismiss = () => setCtxMenu(null);
    window.addEventListener('mousedown', dismiss, true);
    return () => window.removeEventListener('mousedown', dismiss, true);
  }, [ctxMenu]);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'n') { e.preventDefault(); onNew?.(); }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [onNew]);

  const load = useCallback(async () => {
    if (!pairId) return;
    try {
      const items = await listSyncedFiles(pairId, currentPath);
      setFiles(items);
    } catch {}
  }, [pairId, currentPath]);

  useEffect(() => {
    setLoading(true);
    load().finally(() => setLoading(false));
    const t = setInterval(load, 5000);
    return () => clearInterval(t);
  }, [load]);

  // breadcrumb segments
  const crumbs: { label: string; path: string }[] = [{ label: serverHost || 'Nextcloud', path: '/' }];
  currentPath.replace(/^\//, '').split('/').filter(Boolean).reduce((acc, p) => {
    const next = acc + '/' + p;
    crumbs.push({ label: p, path: next });
    return next;
  }, '');

  const localGb = files.reduce((s, f) => s + (f.size ?? 0), 0) / 1e9;

  return (
    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden', background: 'var(--cream)' }}>
      {/* breadcrumb / actions row */}
      <div style={{ height: 52, flexShrink: 0, padding: '0 22px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', borderBottom: '1px solid var(--hairline)' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 13 }}>
          {crumbs.map((c, i) => (
            <React.Fragment key={c.path}>
              {i > 0 && <Icon name="chevron" size={11} color="var(--ink-muted)" />}
              {i < crumbs.length - 1
                ? <button onClick={() => { onPathChange(c.path); }} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--ink-muted)', fontSize: 13 }}>{c.label}</button>
                : <span style={{ color: 'var(--ink)', fontWeight: 600, fontFamily: 'var(--body)', fontSize: 14, letterSpacing: '-0.02em', marginLeft: 4 }}>{c.label}</span>
              }
            </React.Fragment>
          ))}
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <FlatBtn icon="plus" label="New" onClick={onNew} />
          <FlatBtn icon="cloud-dl" label="Make available offline" />
          <FlatBtn icon="share" label="Share" primary onClick={() => {
            if (selected !== null) onShare(files[selected].path);
          }} disabled={selected === null} />
        </div>
      </div>

      {/* table */}
      <div style={{ flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
        <div style={{ display: 'grid', gridTemplateColumns: '38px 1fr 110px 110px 160px 80px', padding: '12px 22px', fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.14em', textTransform: 'uppercase', color: 'var(--ink-muted)', borderBottom: '1px solid var(--hairline)' }}>
          <span/><span>Name</span><span>Size</span><span>Items</span><span>Modified</span>
          <span style={{ textAlign: 'right' }}>Status</span>
        </div>
        <div style={{ flex: 1, overflowY: 'auto', padding: '4px 0 12px' }}>
          {loading && files.length === 0 && <p style={{ padding: '24px 22px', textAlign: 'center', color: 'var(--ink-muted)', fontFamily: 'var(--body)', fontSize: 13 }}>Loading…</p>}
          {!loading && files.length === 0 && <p style={{ padding: '24px 22px', textAlign: 'center', color: 'var(--ink-muted)', fontFamily: 'var(--body)', fontSize: 13 }}>This folder is empty.</p>}
          {files.map((f, i) => {
            const kind = f.is_dir ? 'folder' : (f.name.split('.').pop()?.toLowerCase() ?? 'file');
            const fileWithKind = { ...f, kind };
            return (
              <FileRowItem
                key={f.path}
                file={fileWithKind}
                selected={selected === i}
                starred={favorites?.has(f.path) ?? false}
                onClick={() => setSelected(i === selected ? null : i)}
                onDblClick={() => { if (f.is_dir) onPathChange(f.path); }}
                onShare={() => onShare(f.path)}
                onToggleStar={onToggleFavorite ? () => onToggleFavorite(f) : undefined}
                onContextMenu={(x, y) => { setSelected(i); setCtxMenu({ x, y, file: fileWithKind }); }}
              />
            );
          })}
        </div>
      </div>

      {/* status bar */}
      <div style={{ height: 32, flexShrink: 0, borderTop: '1px solid var(--hairline)', background: 'var(--paper)', display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '0 20px', fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)', letterSpacing: '0.04em' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
          <span>{files.length} items · {selected !== null ? '1 selected' : 'nothing selected'}</span>
          <span>·</span>
          <span>{localGb.toFixed(1)} GB local</span>
          {syncStatus?.status === 'syncing' && (
            <>
              <span>·</span>
              <span style={{ color: 'var(--clay)' }}>
                syncing {syncStatus.active_file_count} file{syncStatus.active_file_count !== 1 ? 's' : ''}
                {syncStatus.eta_seconds != null && ` · ETA ${syncStatus.eta_seconds > 60 ? `${Math.round(syncStatus.eta_seconds / 60)}m` : `${syncStatus.eta_seconds}s`}`}
              </span>
            </>
          )}
          {syncStatus?.last_sync_at && (
            <>
              <span>·</span>
              <span>last sync {new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' }).format(new Date(syncStatus.last_sync_at))}</span>
            </>
          )}
        </div>
      </div>

      {ctxMenu && createPortal(
        <ContextMenu
          x={ctxMenu.x}
          y={ctxMenu.y}
          file={ctxMenu.file}
          starred={favorites?.has(ctxMenu.file.path) ?? false}
          serverUrl={serverHost ? `https://${serverHost}` : ''}
          onShare={() => { setCtxMenu(null); onShare(ctxMenu.file.path); }}
          onToggleStar={onToggleFavorite ? () => { setCtxMenu(null); onToggleFavorite(ctxMenu.file); } : undefined}
          onClose={() => setCtxMenu(null)}
        />,
        document.body,
      )}
    </div>
  );
}

function FileRowItem({ file, selected, starred, onClick, onDblClick, onShare, onToggleStar, onContextMenu }: {
  file: FileStatusDto & { kind: string };
  selected: boolean;
  starred: boolean;
  onClick: () => void;
  onDblClick: () => void;
  onShare: () => void;
  onToggleStar?: () => void;
  onContextMenu?: (x: number, y: number) => void;
}) {
  const [h, setH] = useState(false);
  return (
    <div onClick={onClick} onDoubleClick={onDblClick}
      onContextMenu={e => { e.preventDefault(); onContextMenu?.(e.clientX, e.clientY); }}
      onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{ display: 'grid', gridTemplateColumns: '38px 1fr 110px 110px 160px 96px', padding: '11px 22px', alignItems: 'center', background: selected ? 'var(--paper-2)' : h ? 'color-mix(in srgb, var(--ink) 4%, transparent)' : 'transparent', cursor: 'pointer', boxShadow: selected ? 'inset 2px 0 0 var(--clay)' : 'none', fontSize: 13.5 }}>
      <FileGlyph kind={file.kind} />
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, minWidth: 0 }}>
        <span style={{ color: 'var(--ink)', fontWeight: file.is_dir ? 500 : 400, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{file.name}</span>
        {(file.share_count ?? 0) > 0 && (
          <span title={`Shared with ${file.share_count}`} style={{ display: 'flex', alignItems: 'center', gap: 3, color: 'var(--ink-muted)', fontSize: 11, fontFamily: 'var(--mono)', flexShrink: 0 }}>
            <Icon name="people" size={12} color="var(--ink-muted)" />
            <span>{file.share_count}</span>
          </span>
        )}
      </div>
      <span style={{ color: 'var(--ink-muted)', fontSize: 12, fontFamily: 'var(--mono)' }}>{fmtBytes(file.size)}</span>
      <span style={{ color: 'var(--ink-muted)', fontSize: 12, fontFamily: 'var(--mono)' }}>{file.item_count != null ? String(file.item_count) : '—'}</span>
      <span style={{ color: 'var(--ink-muted)', fontSize: 12 }}>{fmtDate(file.mtime)}</span>
      <span style={{ display: 'flex', justifyContent: 'flex-end', alignItems: 'center', gap: 5 }}>
        <StatusDot status={file.status} />
        {(h || selected) && onToggleStar && (
          <button onClick={(e) => { e.stopPropagation(); onToggleStar(); }} title={starred ? 'Unstar' : 'Star'}
            style={{ background: 'transparent', border: 'none', cursor: 'pointer', padding: 3, display: 'flex', borderRadius: 4, color: starred ? 'var(--clay)' : 'var(--ink-muted)' }}>
            <Icon name="star" size={13} color={starred ? 'var(--clay)' : 'var(--ink-muted)'} strokeWidth={starred ? 2 : 1.6} />
          </button>
        )}
        {(h || selected) && (
          <button onClick={(e) => { e.stopPropagation(); onShare(); }}
            style={{ background: 'transparent', border: 'none', cursor: 'pointer', padding: 3, color: 'var(--ink-muted)', display: 'flex', borderRadius: 4 }}>
            <Icon name="share" size={13} />
          </button>
        )}
      </span>
    </div>
  );
}

// ── Context Menu ──────────────────────────────────────────────────────────────

function ContextMenu({ x, y, file, starred, serverUrl, onShare, onToggleStar, onClose }: {
  x: number;
  y: number;
  file: FileStatusDto & { kind: string };
  starred: boolean;
  serverUrl: string;
  onShare: () => void;
  onToggleStar?: () => void;
  onClose: () => void;
}) {
  const menuRef = useRef<HTMLDivElement>(null);

  // Clamp to viewport so the menu never clips off-screen
  const MENU_W = 220, MENU_H = 248;
  const left = Math.min(x, window.innerWidth - MENU_W - 8);
  const top  = Math.min(y, window.innerHeight - MENU_H - 8);

  type MenuItem =
    | { kind: 'item'; icon: import('./shared').IconName; label: string; onClick: () => void; danger?: boolean }
    | { kind: 'sep' };

  const items: MenuItem[] = [
    {
      kind: 'item', icon: 'folder-plus', label: 'Open',
      onClick: () => { shellOpen(file.path).catch(() => {}); onClose(); },
    },
    {
      kind: 'item', icon: 'globe', label: 'View on server',
      onClick: () => {
        if (serverUrl) shellOpen(`${serverUrl}/apps/files/?dir=${encodeURIComponent(file.path)}`).catch(() => {});
        onClose();
      },
    },
    { kind: 'sep' },
    {
      kind: 'item', icon: 'share', label: 'Share…',
      onClick: onShare,
    },
    {
      kind: 'item', icon: 'link', label: 'Copy link',
      onClick: () => {
        if (serverUrl) navigator.clipboard.writeText(`${serverUrl}/apps/files/?dir=${encodeURIComponent(file.path)}`).catch(() => {});
        onClose();
      },
    },
    { kind: 'sep' },
    {
      kind: 'item', icon: 'star', label: starred ? 'Unstar' : 'Star',
      onClick: () => { onToggleStar?.(); onClose(); },
    },
    {
      kind: 'item', icon: 'cloud-dl', label: 'Make available offline',
      onClick: () => { onClose(); },
    },
    { kind: 'sep' },
    {
      kind: 'item', icon: 'trash', label: 'Move to trash',
      danger: true,
      onClick: () => { onClose(); },
    },
  ];

  return (
    <div
      ref={menuRef}
      onMouseDown={e => e.stopPropagation()}
      style={{ position: 'fixed', left, top, width: MENU_W, background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', boxShadow: '0 8px 24px rgba(0,0,0,0.12)', padding: '4px 0', zIndex: 9999, fontFamily: 'var(--body)', fontSize: 13 }}>
      {items.map((item, i) => {
        if (item.kind === 'sep') {
          return <div key={i} style={{ height: 1, background: 'var(--hairline)', margin: '4px 0' }} />;
        }
        return (
          <CtxItem key={i} icon={item.icon} label={item.label} danger={item.danger} onClick={item.onClick} />
        );
      })}
    </div>
  );
}

function CtxItem({ icon, label, danger, onClick }: {
  icon: import('./shared').IconName;
  label: string;
  danger?: boolean;
  onClick: () => void;
}) {
  const [h, setH] = useState(false);
  const color = danger ? 'var(--danger, #d94f2e)' : 'var(--ink)';
  return (
    <button
      onClick={onClick}
      onMouseEnter={() => setH(true)}
      onMouseLeave={() => setH(false)}
      style={{ width: '100%', display: 'flex', alignItems: 'center', gap: 10, padding: '7px 14px', background: h ? 'var(--paper-2)' : 'transparent', border: 'none', cursor: 'pointer', color, textAlign: 'left', transition: 'background 0.08s' }}>
      <Icon name={icon} size={14} color={color} />
      <span>{label}</span>
    </button>
  );
}
