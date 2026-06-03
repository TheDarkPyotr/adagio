import React, { useEffect, useState } from 'react';
import { open as shellOpen } from '@tauri-apps/plugin-shell';
import { getAllWebviewWindows } from '@tauri-apps/api/webviewWindow';
import { MarkSlur, Icon } from './shared';
import { getStatus, getActivityLog, pauseSync, resumeSync, listPairs, listAccounts } from '../tauri';
import type { SyncStatusDto, ActivityEntryDto } from '../tauri';

export default function TrayPopover() {
  const [status, setStatus] = useState<SyncStatusDto | null>(null);
  const [activity, setActivity] = useState<ActivityEntryDto[]>([]);
  const [localRoot, setLocalRoot] = useState<string | null>(null);
  const [serverUrl, setServerUrl] = useState<string | null>(null);
  const [pausing, setPausing] = useState(false);

  useEffect(() => {
    const loadAll = () => {
      getStatus().then(setStatus).catch(() => {});
      getActivityLog(3).then(setActivity).catch(() => {});
    };
    loadAll();
    const t = setInterval(loadAll, 3000);
    return () => clearInterval(t);
  }, []);

  useEffect(() => {
    listPairs().then(ps => { if (ps[0]) setLocalRoot(ps[0].local_root); }).catch(() => {});
    listAccounts().then(accs => { if (accs[0]) setServerUrl(accs[0].server_url); }).catch(() => {});
  }, []);

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

  const lastSyncFmt = status?.last_sync_at
    ? new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit' }).format(new Date(status.last_sync_at))
    : null;

  const totalGb = (status?.total_bytes ?? 0) / 1e9;

  return (
    <div style={{ width: 380, background: 'var(--cream)', fontFamily: 'var(--body)', color: 'var(--ink)', display: 'flex', flexDirection: 'column', height: '100vh', overflow: 'hidden' }}>

      {/* Header */}
      <div style={{ padding: '14px 16px', borderBottom: '1px solid var(--hairline)', display: 'flex', alignItems: 'center', justifyContent: 'space-between', background: 'var(--paper)', flexShrink: 0 }}>
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
          Recent
        </div>
        {activity.length === 0 ? (
          <div style={{ padding: '12px 16px', fontSize: 12.5, color: 'var(--ink-muted)' }}>No recent activity.</div>
        ) : (
          activity.map(entry => (
            <RecentRow key={entry.id} entry={entry} />
          ))
        )}
      </div>

      {/* Quick actions */}
      <div style={{ borderTop: '1px solid var(--hairline)', padding: 8, flexShrink: 0 }}>
        <ActionRow
          icon="folder-plus"
          label="Open Adagio folder"
          kbd="⌘O"
          onClick={() => localRoot && shellOpen(localRoot).catch(() => {})}
          disabled={!localRoot}
        />
        <ActionRow
          icon="globe"
          label="Open in browser"
          kbd="⌘B"
          onClick={() => serverUrl && shellOpen(serverUrl).catch(() => {})}
          disabled={!serverUrl}
        />
        <ActionRow
          icon="sync"
          label={isPaused ? 'Resume syncing' : 'Pause syncing'}
          kbd="⌘P"
          onClick={handlePauseResume}
          disabled={pausing}
        />
        <ActionRow
          icon="settings"
          label="Preferences…"
          kbd="⌘,"
          onClick={async () => {
            const wins = await getAllWebviewWindows();
            const main = wins.find(w => w.label === 'main');
            if (main) { await main.show(); await main.setFocus(); }
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

function RecentRow({ entry }: { entry: ActivityEntryDto }) {
  const iconName = entry.kind === 'share' ? 'share' : entry.kind === 'sync' ? 'sync' : 'pencil';
  const timeAgo = formatAgo(entry.at);
  const label = entry.kind === 'sync' && entry.target.includes('/')
    ? `files in ${entry.target.split('/').slice(-2, -1)[0] || entry.target}/`
    : entry.target.split('/').pop() ?? entry.target;

  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '7px 16px' }}>
      <div style={{ width: 24, height: 24, borderRadius: 12, background: 'var(--paper-2)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
        <Icon name={iconName} size={12} color="var(--ink-soft)" />
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 12.5, fontWeight: 500, color: 'var(--ink)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{label}</div>
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
