import React, { useEffect, useState, useCallback } from 'react';
import { open as shellOpen } from '@tauri-apps/plugin-shell';
import { getAllWebviewWindows, getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { MarkSlur, Icon } from './shared';
import { getStatus, listSyncedFiles, pauseSync, resumeSync, listPairs, listAccounts, getPlatform } from '../tauri';
import type { SyncStatusDto, FileStatusDto, PairDto } from '../tauri';

interface RecentFile {
  name: string;
  parentFolder: string | null;
  mtime: number;
  status: FileStatusDto['status'];
  fullPath: string;
}

export default function TrayPopover() {
  const [status, setStatus] = useState<SyncStatusDto | null>(null);
  const [recentFiles, setRecentFiles] = useState<RecentFile[]>([]);
  const [pairs, setPairs] = useState<PairDto[]>([]);
  const [localRoot, setLocalRoot] = useState<string | null>(null);
  const [serverUrl, setServerUrl] = useState<string | null>(null);
  const [pausing, setPausing] = useState(false);
  const [platform, setPlatform] = useState<'linux' | 'macos' | 'windows'>('macos');

  const [activePairId, setActivePairId] = useState<string | null>(null);

  // Fetch recent files from the local filesystem via listSyncedFiles (same
  // source as the desktop file browser). Collects files from all pairs sorted
  // by modification time, showing the 5 most recently touched.
  const loadRecentFiles = useCallback(async (ps: PairDto[], filterPairId?: string | null) => {
    // If a specific pair is active (desktop switched accounts), show only that pair's files.
    // Otherwise show the N most recently touched files across all pairs.
    const activePairs = filterPairId ? ps.filter(p => p.id === filterPairId) : ps;
    const all: RecentFile[] = [];
    for (const pair of activePairs) {
      try {
        const files = await listSyncedFiles(pair.id, '/');
        for (const f of files) {
          if (!f.is_dir && f.mtime != null) {
            // f.path starts with '/', pair.local_root has no trailing slash
            const fullPath = pair.local_root.replace(/\/$/, '') + f.path;
            all.push({
              name: f.name,
              parentFolder: null,
              mtime: f.mtime,
              status: f.status,
              fullPath,
            });
          }
        }
      } catch {}
    }
    all.sort((a, b) => b.mtime - a.mtime);
    setRecentFiles(all.slice(0, 5));
  }, []);

  useEffect(() => {
    const loadAll = () => { getStatus().then(setStatus).catch(() => {}); };
    loadAll();
    const t = setInterval(loadAll, 3000);
    return () => clearInterval(t);
  }, []);

  useEffect(() => {
    listPairs().then(ps => {
      setPairs(ps);
      if (ps[0]) setLocalRoot(ps[0].local_root);
      loadRecentFiles(ps, activePairId).catch(() => {});
    }).catch(() => {});
    listAccounts().then(accs => { if (accs[0]) setServerUrl(accs[0].server_url); }).catch(() => {});
    getPlatform().then(setPlatform).catch(() => {});
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  // Reload pairs and files whenever the tray popover becomes visible.
  // This is the most reliable way to get fresh data after account switches,
  // since it fires unconditionally on every open regardless of events.
  useEffect(() => {
    const handleVisible = () => {
      if (document.visibilityState !== 'visible') return;
      listPairs().then(ps => {
        setPairs(ps);
        // Use the last-known activePairId (from account-changed event) or show all.
        // pairId read from ref below (activePairIdRef kept in sync via useEffect)
        const pairId = activePairIdRef.current;
        const activePair = pairId ? ps.find(p => p.id === pairId) : ps[0];
        if (activePair) setLocalRoot(activePair.local_root);
        loadRecentFiles(ps, pairId).catch(() => {});
      }).catch(() => {});
    };
    document.addEventListener('visibilitychange', handleVisible);
    return () => document.removeEventListener('visibilitychange', handleVisible);
  }, [loadRecentFiles]);

  // Keep a ref to activePairId so the visibility handler always sees the latest value.
  const activePairIdRef = React.useRef<string | null>(null);
  useEffect(() => { activePairIdRef.current = activePairId; }, [activePairId]);

  // React to account switches from the main window (fires instantly when desktop switches).
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebviewWindow()
      .listen<{ accountId: string; pairId: string | null }>('adagio://active-account-changed', ({ payload }) => {
        setActivePairId(payload.pairId);
        listPairs().then(ps => {
          const activePair = ps.find(p => p.id === payload.pairId)
            ?? ps.find(p => p.account_id === payload.accountId)
            ?? ps[0];
          if (activePair) setLocalRoot(activePair.local_root);
          loadRecentFiles(ps, payload.pairId).catch(() => {});
        }).catch(() => {});
      })
      .then(fn => { unlisten = fn; })
      .catch(() => {});
    return () => { unlisten?.(); };
  }, [loadRecentFiles]);

  const handlePauseResume = async () => {
    setPausing(true);
    try {
      if (status?.status === 'paused') await resumeSync();
      else await pauseSync();
      getStatus().then(setStatus).catch(() => {});
    } catch {}
    setPausing(false);
  };

  const isPaused = status?.status === 'paused';
  const isSyncing = status?.status === 'syncing';

  const isMaintenance = status?.status === 'maintenance';
  const isUnreachable = status?.status === 'unreachable';

  const statusLabel = isSyncing
    ? `SYNCING ${status!.active_file_count} FILE${status!.active_file_count !== 1 ? 'S' : ''}`
    : isPaused ? 'PAUSED'
    : status?.status === 'error' ? 'ERROR'
    : isMaintenance ? 'MAINTENANCE'
    : isUnreachable ? 'UNREACHABLE'
    : 'IN SYNC';

  const statusColor = isSyncing ? 'var(--warn)'
    : isPaused ? 'var(--ink-muted)'
    : status?.status === 'error' ? 'var(--danger)'
    : isMaintenance ? 'var(--warn)'
    : isUnreachable ? 'var(--danger)'
    : 'var(--good)';

  // last_sync_at is now set by the daemon after each sync cycle (including VFS).
  // Never fall back to file mtime — that's when a file was modified, not when sync ran.
  const lastSyncMs = status?.last_sync_at ?? null;
  const lastSyncFmt = lastSyncMs
    ? new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' }).format(new Date(lastSyncMs))
    : null;

  const totalGb = (status?.total_bytes ?? 0) / 1e9;
  const mod = platform === 'macos' ? '⌘' : 'Ctrl+';

  return (
    <div style={{ width: 380, background: 'var(--cream)', fontFamily: 'var(--body)', color: 'var(--ink)', display: 'flex', flexDirection: 'column', height: '100vh', overflow: 'hidden' }}>

      {/* Header — drag region so the popover can be repositioned */}
      <div
        data-tauri-drag-region
        style={{ padding: '14px 16px', borderBottom: '1px solid var(--hairline)', display: 'flex', alignItems: 'center', justifyContent: 'space-between', background: 'var(--paper)', flexShrink: 0, cursor: 'move', userSelect: 'none' }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <MarkSlur size={18} fill="var(--ink)" accent="var(--clay)" />
          <span style={{ fontWeight: 500, fontSize: 16, letterSpacing: '-0.05em' }}>adagio</span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
          <div style={{ width: 6, height: 6, borderRadius: 3, background: statusColor }} />
          <span style={{ fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.14em', color: statusColor }}>{statusLabel}</span>
        </div>
      </div>

      {/* Status block */}
      <div style={{ padding: '16px 16px 14px', borderBottom: '1px solid var(--hairline)', flexShrink: 0 }}>
        <StatusHeadline status={status} />
        <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)', marginTop: 6, letterSpacing: '0.04em' }}>
          {lastSyncFmt ? `last sync ${lastSyncFmt}` : 'never synced'}
          {totalGb > 0 && ` · ${totalGb.toFixed(1)} GB`}
        </div>
      </div>

      {/* Recent files */}
      <div style={{ flex: 1, overflow: 'hidden' }}>
        <div style={{ padding: '10px 16px 4px', fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.16em', textTransform: 'uppercase', color: 'var(--ink-muted)' }}>
          Recent files
        </div>
        {recentFiles.length === 0 ? (
          <div style={{ padding: '12px 16px', fontSize: 12.5, color: 'var(--ink-muted)' }}>No files yet.</div>
        ) : (
          recentFiles.map((f, i) => (
            <RecentRow key={i} file={f} />
          ))
        )}
      </div>

      {/* Quick actions */}
      <div style={{ borderTop: '1px solid var(--hairline)', padding: 8, flexShrink: 0 }}>
        <ActionRow
          icon="folder-plus"
          label="Open Adagio folder"
          kbd={`${mod}O`}
          onClick={() => localRoot && shellOpen(localRoot).catch(() => {})}
          disabled={!localRoot}
        />
        <ActionRow
          icon="globe"
          label="Open in browser"
          kbd={`${mod}B`}
          onClick={() => serverUrl && shellOpen(serverUrl).catch(() => {})}
          disabled={!serverUrl}
        />
        <ActionRow
          icon="sync"
          label={isPaused ? 'Resume syncing' : 'Pause syncing'}
          kbd={`${mod}P`}
          onClick={handlePauseResume}
          disabled={pausing}
        />
        <ActionRow
          icon="settings"
          label="Preferences…"
          kbd={`${mod},`}
          onClick={async () => {
            const wins = await getAllWebviewWindows();
            const main = wins.find(w => w.label === 'main');
            if (main) { await main.show(); await main.setFocus(); await main.emit('adagio://open-settings', {}); }
          }}
        />
      </div>

      {/* Footer */}
      <div style={{ padding: '10px 16px', borderTop: '1px solid var(--hairline)', background: 'var(--paper)', display: 'flex', alignItems: 'center', justifyContent: 'space-between', flexShrink: 0 }}>
        <span style={{ fontFamily: 'var(--mono)', fontSize: 10.5, color: 'var(--ink-muted)' }}>v1.0 · allegretto</span>
        <button
          onClick={async () => {
            const wins = await getAllWebviewWindows();
            await Promise.all(wins.map(w => w.close()));
          }}
          style={{ background: 'none', border: 'none', cursor: 'pointer', fontFamily: 'var(--mono)', fontSize: 10.5, color: 'var(--ink-muted)', padding: 0 }}>
          Quit Adagio
        </button>
      </div>
    </div>
  );
}

function StatusHeadline({ status }: { status: SyncStatusDto | null }) {
  if (status?.status === 'syncing') {
    return (
      <div style={{ fontSize: 28, fontWeight: 500, lineHeight: 1.1, letterSpacing: '-0.04em' }}>
        Syncing{' '}
        <span style={{ fontFamily: 'var(--serif)', fontWeight: 400, fontStyle: 'italic' }}>
          {status.active_file_count} file{status.active_file_count !== 1 ? 's' : ''}
        </span>
      </div>
    );
  }
  if (status?.status === 'paused') {
    return <div style={{ fontSize: 28, fontWeight: 500, lineHeight: 1.1, letterSpacing: '-0.04em' }}>Paused</div>;
  }
  if (status?.status === 'error') {
    return <div style={{ fontSize: 28, fontWeight: 500, lineHeight: 1.1, letterSpacing: '-0.04em', color: 'var(--danger)' }}>Error</div>;
  }
  if (status?.status === 'maintenance') {
    return <div style={{ fontSize: 28, fontWeight: 500, lineHeight: 1.1, letterSpacing: '-0.04em', color: 'var(--warn)' }}>Maintenance</div>;
  }
  if (status?.status === 'unreachable') {
    return <div style={{ fontSize: 28, fontWeight: 500, lineHeight: 1.1, letterSpacing: '-0.04em', color: 'var(--danger)' }}>Unreachable</div>;
  }
  return (
    <div style={{ fontSize: 28, fontWeight: 500, lineHeight: 1.1, letterSpacing: '-0.04em' }}>
      Up to{' '}
      <span style={{ fontFamily: 'var(--serif)', fontWeight: 400, fontStyle: 'italic' }}>date</span>
    </div>
  );
}

function RecentRow({ file }: { file: RecentFile }) {
  const [hovered, setHovered] = useState(false);
  const isConflict = file.status === 'conflict';
  const iconName = isConflict ? 'warn' : 'file';
  const iconColor = isConflict ? 'var(--danger)' : 'var(--ink-soft)';
  const timeAgo = formatAgo(file.mtime);

  return (
    <div
      onClick={() => shellOpen(file.fullPath).catch(() => {})}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '7px 16px', cursor: 'pointer', background: hovered ? 'var(--paper-2)' : 'transparent', transition: 'background 0.1s' }}
    >
      <div style={{ width: 24, height: 24, borderRadius: 12, background: isConflict ? 'color-mix(in srgb, var(--danger) 12%, transparent)' : 'var(--paper-2)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
        <Icon name={iconName} size={12} color={iconColor} />
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 12.5, fontWeight: 500, color: 'var(--ink)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{file.name}</div>
        <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)' }}>{timeAgo}</div>
      </div>
    </div>
  );
}

function ActionRow({ icon, label, kbd, onClick, disabled }: {
  icon: import('./shared').IconName;
  label: string;
  kbd: string;
  onClick: () => void;
  disabled?: boolean;
}) {
  const [h, setH] = useState(false);
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      onMouseEnter={() => setH(true)}
      onMouseLeave={() => setH(false)}
      style={{ width: '100%', display: 'flex', alignItems: 'center', gap: 12, padding: '8px', borderRadius: 6, background: h && !disabled ? 'var(--paper-2)' : 'transparent', border: 'none', cursor: disabled ? 'default' : 'pointer', opacity: disabled ? 0.4 : 1, transition: 'background 0.1s' }}>
      <Icon name={icon} size={14} color="var(--ink-soft)" />
      <span style={{ flex: 1, fontSize: 13, color: 'var(--ink)', textAlign: 'left' }}>{label}</span>
      <span style={{ fontFamily: 'var(--mono)', fontSize: 10.5, color: 'var(--ink-muted)' }}>{kbd}</span>
    </button>
  );
}

function formatAgo(ts: number): string {
  const diffMs = Date.now() - ts;
  const mins = Math.floor(diffMs / 60000);
  if (mins < 1) return 'just now';
  if (mins < 60) return `${mins} min ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h ago`;
  return `${Math.floor(hrs / 24)}d ago`;
}
