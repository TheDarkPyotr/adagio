import React, { useState, useEffect, useRef, useCallback } from 'react';
import { Icon, SpinDot } from './shared';
import type { PairDto, AccountDto, SyncStatusDto } from '../tauri';
import { createPair, deletePair, getStatus, triggerSync } from '../tauri';

export default function PairsScene({ pairs, account, onBack, onPairsChange }: {
  pairs: PairDto[];
  account: AccountDto | null;
  onBack: () => void;
  onPairsChange: (pairs: PairDto[]) => void;
}) {
  const [showAdd, setShowAdd] = useState(false);
  const [localRoot, setLocalRoot] = useState('');
  const [remoteRoot, setRemoteRoot] = useState('/');
  const [vfsEnabled, setVfsEnabled] = useState(false);
  const [adding, setAdding] = useState(false);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [addError, setAddError] = useState<string | null>(null);

  // Sync state per pair: tracks which pair is currently syncing.
  const [syncingPair, setSyncingPair] = useState<string | null>(null);
  // Start timestamp for elapsed-time display.
  const syncStartRef = useRef<number | null>(null);
  const [syncElapsed, setSyncElapsed] = useState(0);
  // Last known duration per pair for ETA estimation.
  const [lastDuration, setLastDuration] = useState<Record<string, number>>({});
  // Poll interval handle.
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);
  // Elapsed timer handle.
  const elapsedRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Clear all timers.
  const clearTimers = useCallback(() => {
    if (pollRef.current)   { clearInterval(pollRef.current);   pollRef.current   = null; }
    if (elapsedRef.current){ clearInterval(elapsedRef.current); elapsedRef.current = null; }
  }, []);

  // Called when a sync cycle finishes: record duration, clear spinners.
  const onSyncDone = useCallback((pairId: string) => {
    if (syncStartRef.current !== null) {
      const duration = (Date.now() - syncStartRef.current) / 1000;
      setLastDuration(prev => ({ ...prev, [pairId]: Math.max(1, duration) }));
      syncStartRef.current = null;
    }
    setSyncingPair(null);
    setSyncElapsed(0);
    clearTimers();
  }, [clearTimers]);

  // Cleanup on unmount.
  useEffect(() => () => clearTimers(), [clearTimers]);

  const handleAdd = async () => {
    if (!account || !localRoot.trim()) return;
    setAdding(true);
    setAddError(null);
    try {
      const pair = await createPair({
        account_id: account.id,
        local_root: localRoot.trim(),
        remote_root: remoteRoot.trim() || '/',
        vfs_enabled: vfsEnabled,
      });
      onPairsChange([...pairs, pair]);
      setShowAdd(false);
      setLocalRoot('');
      setRemoteRoot('/');
      setVfsEnabled(false);
    } catch (e: unknown) {
      setAddError(e instanceof Error ? e.message : 'Failed to create pair.');
    }
    setAdding(false);
  };

  const handleDelete = async (id: string) => {
    setDeleting(id);
    try {
      await deletePair(id, false);
      onPairsChange(pairs.filter(p => p.id !== id));
    } catch {}
    setDeleting(null);
  };

  const handleSync = async (id: string) => {
    // Trigger the cycle (returns immediately).
    try { await triggerSync(id); } catch { return; }

    // Start spinner and elapsed counter.
    syncStartRef.current = Date.now();
    setSyncingPair(id);
    setSyncElapsed(0);
    clearTimers();

    // Elapsed ticker — updates every second.
    elapsedRef.current = setInterval(() => {
      setSyncElapsed(s => s + 1);
    }, 1000);

    // Poll getStatus() every 600ms until the engine returns to idle.
    // We need fast polling because sync cycles can complete in a few seconds.
    pollRef.current = setInterval(async () => {
      try {
        const status = await getStatus();
        if (status.status === 'idle' || status.status === 'paused' || status.status === 'error') {
          onSyncDone(id);
        }
      } catch {
        onSyncDone(id);
      }
    }, 600);
  };

  return (
    <div style={{ flex: 1, display: 'flex', overflow: 'hidden', background: 'var(--cream)' }}>
      {/* left nav */}
      <div style={{ width: 220, flexShrink: 0, borderRight: '1px solid var(--hairline)', background: 'var(--paper)', display: 'flex', flexDirection: 'column', padding: '20px 10px' }}>
        <button onClick={onBack}
          style={{ display: 'flex', alignItems: 'center', gap: 8, background: 'transparent', border: 'none', cursor: 'pointer', color: 'var(--ink-muted)', fontSize: 13, padding: '6px 10px', borderRadius: 'var(--r-2)', marginBottom: 16, textAlign: 'left' }}>
          <div style={{ transform: 'rotate(180deg)', display: 'flex' }}>
            <Icon name="arrow-r" size={13} color="var(--ink-muted)" />
          </div>
          Back to settings
        </button>
        <div style={{ padding: '2px 10px 8px', fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.16em', textTransform: 'uppercase', color: 'var(--ink-muted)' }}>Sync Pairs</div>
        <div style={{ padding: '8px 10px', fontSize: 13, color: 'var(--ink-soft)', lineHeight: 1.5 }}>
          Pairs link a local folder to a Nextcloud path. Files sync in both directions automatically.
        </div>
      </div>

      {/* content */}
      <div style={{ flex: 1, overflowY: 'auto', padding: '36px 52px' }}>
        <div style={{ maxWidth: 560 }}>
          <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 24 }}>
            <h2 style={SECTION_H2}>Sync pairs</h2>
            {!showAdd && (
              <button onClick={() => setShowAdd(true)}
                style={{ display: 'flex', alignItems: 'center', gap: 6, background: 'var(--ink)', color: 'var(--cream)', border: 'none', padding: '8px 16px', borderRadius: 'var(--r-pill)', fontSize: 12.5, fontWeight: 500, cursor: 'pointer' }}>
                <Icon name="plus" size={13} color="var(--cream)" />
                Add pair
              </button>
            )}
          </div>

          {pairs.length === 0 && !showAdd && (
            <div style={{ padding: '48px 0', textAlign: 'center', color: 'var(--ink-muted)', fontSize: 13 }}>
              <div style={{ marginBottom: 8, fontSize: 32 }}>⇄</div>
              No sync pairs yet. Add one to start syncing.
            </div>
          )}

          {pairs.map(pair => (
            <PairRow
              key={pair.id}
              pair={pair}
              syncing={syncingPair === pair.id}
              syncElapsed={syncingPair === pair.id ? syncElapsed : 0}
              lastDuration={lastDuration[pair.id] ?? null}
              deleting={deleting === pair.id}
              onSync={() => handleSync(pair.id)}
              onDelete={() => handleDelete(pair.id)}
            />
          ))}

          {showAdd && (
            <div style={{ marginTop: pairs.length > 0 ? 24 : 0, padding: 20, background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-3)' }}>
              <div style={FIELD_LABEL}>Local folder</div>
              <input
                value={localRoot}
                onChange={e => setLocalRoot(e.target.value)}
                placeholder="~/Adagio"
                style={INPUT_STYLE}
              />
              <div style={{ ...FIELD_LABEL, marginTop: 14 }}>Remote path (Nextcloud)</div>
              <input
                value={remoteRoot}
                onChange={e => setRemoteRoot(e.target.value)}
                placeholder="/Adagio"
                style={INPUT_STYLE}
              />
              <label style={{ display: 'flex', alignItems: 'center', gap: 10, marginTop: 16, cursor: 'pointer', userSelect: 'none' }}>
                <div
                  onClick={() => setVfsEnabled(v => !v)}
                  style={{
                    width: 36, height: 20, borderRadius: 10, background: vfsEnabled ? 'var(--ink)' : 'var(--hairline)',
                    position: 'relative', transition: 'background 0.15s', flexShrink: 0, cursor: 'pointer',
                  }}
                >
                  <div style={{
                    position: 'absolute', top: 3, left: vfsEnabled ? 19 : 3, width: 14, height: 14,
                    borderRadius: '50%', background: 'var(--paper)', transition: 'left 0.15s',
                    boxShadow: '0 1px 3px rgba(0,0,0,0.18)',
                  }} />
                </div>
                <div>
                  <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--ink)' }}>VFS on-demand</div>
                  <div style={{ fontSize: 11.5, color: 'var(--ink-muted)', marginTop: 2 }}>
                    Files appear instantly as placeholders; content downloads only when opened.
                  </div>
                </div>
              </label>
              {addError && (
                <div style={{ fontSize: 12, color: 'var(--clay)', marginTop: 10 }}>{addError}</div>
              )}
              <div style={{ display: 'flex', gap: 8, marginTop: 16, justifyContent: 'flex-end' }}>
                <button onClick={() => { setShowAdd(false); setAddError(null); setVfsEnabled(false); }}
                  style={{ background: 'transparent', border: '1px solid var(--hairline)', padding: '7px 16px', borderRadius: 'var(--r-pill)', fontSize: 12.5, cursor: 'pointer', color: 'var(--ink)' }}>
                  Cancel
                </button>
                <button onClick={handleAdd} disabled={adding || !localRoot.trim()}
                  style={{ background: 'var(--ink)', color: 'var(--cream)', border: 'none', padding: '7px 18px', borderRadius: 'var(--r-pill)', fontSize: 12.5, fontWeight: 500, cursor: adding ? 'wait' : 'pointer', opacity: adding || !localRoot.trim() ? 0.5 : 1, transition: 'opacity 0.12s' }}>
                  {adding ? 'Adding…' : 'Add pair'}
                </button>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

// ── PairRow ───────────────────────────────────────────────────────────────────

function PairRow({ pair, syncing, syncElapsed, lastDuration, deleting, onSync, onDelete }: {
  pair: PairDto;
  syncing: boolean;
  syncElapsed: number;
  lastDuration: number | null;
  deleting: boolean;
  onSync: () => void;
  onDelete: () => void;
}) {
  const [h, setH] = useState(false);

  // ETA label: time remaining based on last known duration.
  const etaLabel = (() => {
    if (!syncing) return null;
    if (lastDuration !== null && lastDuration > 0) {
      const remaining = Math.max(0, Math.round(lastDuration - syncElapsed));
      if (remaining > 0) return `~${remaining}s left`;
    }
    // No prior data — just show elapsed.
    return syncElapsed > 0 ? `${syncElapsed}s` : null;
  })();

  return (
    <div onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{ padding: '16px 18px', background: h ? 'var(--paper-2)' : 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-3)', marginBottom: 10, transition: 'background 0.12s' }}>
      <div style={{ display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', gap: 16 }}>
        <div style={{ minWidth: 0 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, marginBottom: 6 }}>
            <PathChip label={pair.local_root} />
            <Icon name="arrow-r" size={12} color="var(--ink-muted)" />
            <PathChip label={pair.remote_root} cloud />
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--ink-muted)', letterSpacing: '0.08em' }}>
              {pair.id.slice(0, 8)}
            </div>
            {pair.vfs_enabled && (
              <div style={{ fontSize: 10, fontFamily: 'var(--mono)', background: 'var(--clay)', color: '#fff', borderRadius: 3, padding: '1px 5px', letterSpacing: '0.06em' }}>
                VFS
              </div>
            )}
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 6, flexShrink: 0 }}>
          {/* Sync button with spinner + ETA */}
          <button onClick={onSync} disabled={syncing}
            style={{
              background: syncing ? 'var(--cream-2)' : 'transparent',
              border: '1px solid var(--hairline)',
              padding: '6px 12px',
              borderRadius: 'var(--r-pill)',
              fontSize: 12,
              cursor: syncing ? 'default' : 'pointer',
              color: syncing ? 'var(--clay)' : 'var(--ink-soft)',
              fontWeight: 500,
              display: 'flex',
              alignItems: 'center',
              gap: 6,
              transition: 'background 0.15s, color 0.15s',
              minWidth: 88,
            }}>
            {syncing
              ? <><SpinDot color="var(--clay)" /> {etaLabel ?? 'Syncing…'}</>
              : <><Icon name="refresh" size={12} color="var(--ink-muted)" /> Sync now</>
            }
          </button>

          <button onClick={onDelete} disabled={!!deleting}
            style={{ background: 'transparent', border: '1px solid var(--hairline)', padding: '6px 10px', borderRadius: 'var(--r-pill)', fontSize: 12, cursor: deleting ? 'wait' : 'pointer', color: 'var(--clay)', opacity: deleting ? 0.5 : 1, display: 'flex', alignItems: 'center', transition: 'opacity 0.12s' }}>
            <Icon name="trash" size={13} color="var(--clay)" />
          </button>
        </div>
      </div>
    </div>
  );
}

// ── Helpers ───────────────────────────────────────────────────────────────────

function PathChip({ label, cloud }: { label: string; cloud?: boolean }) {
  const display = label.replace(/^\/home\/[^/]+/, '~');
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 4, background: 'var(--cream-2)', padding: '3px 8px', borderRadius: 'var(--r-2)', maxWidth: 180, overflow: 'hidden' }}>
      <Icon name={cloud ? 'cloud-dl' : 'folder-plus'} size={11} color="var(--ink-muted)" />
      <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{display}</span>
    </div>
  );
}

const SECTION_H2: React.CSSProperties = { fontFamily: 'var(--body)', fontWeight: 500, fontSize: 22, letterSpacing: '-0.04em', margin: 0 };
const FIELD_LABEL: React.CSSProperties = { fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.16em', textTransform: 'uppercase', color: 'var(--ink-muted)', marginBottom: 6 };
const INPUT_STYLE: React.CSSProperties = { width: '100%', boxSizing: 'border-box', background: 'var(--cream)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', padding: '8px 12px', fontSize: 13, color: 'var(--ink)', fontFamily: 'var(--mono)', outline: 'none' };
