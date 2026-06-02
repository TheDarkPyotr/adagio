import React, { useState, useEffect, useRef, useCallback } from 'react';
import { Icon, SpinDot } from './shared';
import type { PairDto, AccountDto, SyncStatusDto } from '../tauri';
import { createPair, deletePair, getStatus, triggerSync, e2eeInit, e2eePair, e2eeDisable } from '../tauri';

export default function PairsScene({ pairs, account, onBack, onPairsChange, daemonState }: {
  pairs: PairDto[];
  account: AccountDto | null;
  onBack: () => void;
  onPairsChange: (pairs: PairDto[]) => void;
  daemonState: 'connected' | 'reconnecting' | 'stopped' | 'failed' | null;
}) {
  const [showAdd, setShowAdd] = useState(false);
  const [localRoot, setLocalRoot] = useState('');
  const [remoteRoot, setRemoteRoot] = useState('/');
  const [vfsEnabled, setVfsEnabled] = useState(false);
  const [adding, setAdding] = useState(false);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [addError, setAddError] = useState<string | null>(null);

  // E2EE modal state
  const [e2eeMnemonic, setE2eeMnemonic] = useState<string | null>(null);
  const [e2eeMnemonicConfirmed, setE2eeMnemonicConfirmed] = useState(false);
  const [e2eeInitError, setE2eeInitError] = useState<string | null>(null);
  const [e2eeInitLoading, setE2eeInitLoading] = useState(false);
  const [e2eePairModal, setE2eePairModal] = useState<string | null>(null); // pair ID waiting for pairing
  const [e2eePairingMnemonic, setE2eePairingMnemonic] = useState('');
  const [e2eePairingError, setE2eePairingError] = useState<string | null>(null);
  const [e2eePairingLoading, setE2eePairingLoading] = useState(false);

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

  const [syncErrors, setSyncErrors] = useState<Record<string, string>>({});

  const clearSyncError = (id: string) =>
    setSyncErrors(prev => { const n = { ...prev }; delete n[id]; return n; });

  const handleSync = async (id: string) => {
    clearSyncError(id);

    // If we already know the daemon is down, fail immediately.
    if (daemonState !== null && daemonState !== 'connected') {
      setSyncErrors(prev => ({ ...prev, [id]: 'Sync daemon is not running.' }));
      return;
    }

    // Race the IPC call against a 4 s timeout so a hung connection still
    // surfaces an error instead of silently spinning forever.
    const timeout = new Promise<never>((_, rej) =>
      setTimeout(() => rej(new Error('Sync daemon is not responding.')), 4000),
    );
    try {
      await Promise.race([triggerSync(id), timeout]);
    } catch (err) {
      setSyncErrors(prev => ({ ...prev, [id]: String(err) }));
      return;
    }

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
        {(daemonState !== 'connected' && daemonState !== null) && (
          <div style={{
            marginBottom: 20, padding: '10px 16px',
            background: 'var(--clay)', color: '#fff',
            borderRadius: 'var(--r-2)', fontSize: 13,
            display: 'flex', alignItems: 'center', gap: 10,
          }}>
            <Icon name="warn" size={15} color="#fff" />
            {daemonState === 'reconnecting'
              ? 'Reconnecting to sync daemon — Sync Now is unavailable.'
              : 'Sync is not running — Sync Now is unavailable.'}
          </div>
        )}
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
              syncDisabled={daemonState !== 'connected'}
              syncError={syncErrors[pair.id] ?? null}
              onSync={() => handleSync(pair.id)}
              onDelete={() => handleDelete(pair.id)}
              onE2eeInit={async () => {
                setE2eeInitError(null);
                setE2eeInitLoading(true);
                try {
                  const mnemonic = await e2eeInit(pair.id);
                  setE2eeMnemonic(mnemonic);
                  setE2eeMnemonicConfirmed(false);
                  onPairsChange(pairs.map(p => p.id === pair.id ? { ...p, e2ee_enabled: true } : p));
                } catch (err) {
                  setE2eeInitError(err instanceof Error ? err.message : String(err));
                } finally {
                  setE2eeInitLoading(false);
                }
              }}
              onE2eePair={() => { setE2eePairModal(pair.id); setE2eePairingMnemonic(''); setE2eePairingError(null); }}
              e2eeInitLoading={e2eeInitLoading}
              onE2eeDisable={pair.e2ee_enabled ? async () => {
                try {
                  await e2eeDisable(pair.id);
                  onPairsChange(pairs.map(p => p.id === pair.id ? { ...p, e2ee_enabled: false } : p));
                } catch {}
              } : undefined}
            />
          ))}

          {/* E2EE init error banner */}
          {e2eeInitError && (
            <div style={{ margin: '8px 0', padding: '10px 16px', background: 'color-mix(in srgb, var(--danger) 10%, var(--paper))', border: '1px solid var(--danger)', borderRadius: 'var(--r-2)', fontSize: 12.5, color: 'var(--danger)', display: 'flex', alignItems: 'flex-start', gap: 10 }}>
              <span style={{ flexShrink: 0, fontWeight: 600 }}>E2EE init failed:</span>
              <span style={{ flex: 1, wordBreak: 'break-word' }}>{e2eeInitError}</span>
              <button onClick={() => setE2eeInitError(null)} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--danger)', flexShrink: 0, padding: 0, fontSize: 14, lineHeight: 1 }}>✕</button>
            </div>
          )}

          {/* E2EE mnemonic display modal */}
          {e2eeMnemonic && !e2eeMnemonicConfirmed && (
            <div style={{ position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.45)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 9999 }}>
              <div style={{ background: 'var(--paper)', borderRadius: 'var(--r-3)', padding: 28, maxWidth: 420, width: '90%', boxShadow: '0 12px 40px rgba(0,0,0,0.18)' }}>
                <div style={{ fontWeight: 600, fontSize: 16, letterSpacing: '-0.03em', marginBottom: 10 }}>Save your recovery mnemonic</div>
                <div style={{ fontSize: 13, color: 'var(--ink-muted)', marginBottom: 18 }}>Write down these 12 words in order. This is the only time they will be shown. You need them to access encrypted files on another device.</div>
                <div style={{ background: 'var(--cream-2)', borderRadius: 'var(--r-2)', padding: '14px 18px', fontFamily: 'var(--mono)', fontSize: 13, letterSpacing: '0.02em', lineHeight: 2, marginBottom: 18 }}>
                  {e2eeMnemonic.split(' ').map((w, i) => (
                    <span key={i} style={{ display: 'inline-block', marginRight: 8 }}><span style={{ color: 'var(--ink-muted)', fontSize: 10 }}>{i + 1}.</span> {w}</span>
                  ))}
                </div>
                <div style={{ display: 'flex', gap: 10, justifyContent: 'flex-end' }}>
                  <button onClick={() => navigator.clipboard.writeText(e2eeMnemonic).catch(() => {})}
                    style={{ background: 'var(--cream-2)', border: '1px solid var(--hairline)', padding: '7px 14px', borderRadius: 'var(--r-pill)', fontSize: 12.5, cursor: 'pointer', color: 'var(--ink)' }}>
                    Copy
                  </button>
                  <button onClick={() => { setE2eeMnemonicConfirmed(true); setE2eeMnemonic(null); }}
                    style={{ background: 'var(--forest)', color: '#fff', border: 'none', padding: '7px 18px', borderRadius: 'var(--r-pill)', fontSize: 12.5, fontWeight: 500, cursor: 'pointer' }}>
                    I have saved this
                  </button>
                </div>
              </div>
            </div>
          )}

          {/* E2EE pairing modal */}
          {e2eePairModal && (
            <div style={{ position: 'fixed', inset: 0, background: 'rgba(0,0,0,0.45)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 9999 }}>
              <div style={{ background: 'var(--paper)', borderRadius: 'var(--r-3)', padding: 28, maxWidth: 420, width: '90%', boxShadow: '0 12px 40px rgba(0,0,0,0.18)' }}>
                <div style={{ fontWeight: 600, fontSize: 16, letterSpacing: '-0.03em', marginBottom: 10 }}>Pair this device</div>
                <div style={{ fontSize: 13, color: 'var(--ink-muted)', marginBottom: 14 }}>Enter the 12-word mnemonic from your other device to access encrypted files.</div>
                <textarea
                  value={e2eePairingMnemonic}
                  onChange={e => setE2eePairingMnemonic(e.target.value)}
                  placeholder="word1 word2 word3 … word12"
                  rows={3}
                  style={{ width: '100%', boxSizing: 'border-box', background: 'var(--cream)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', padding: '8px 12px', fontSize: 13, fontFamily: 'var(--mono)', outline: 'none', resize: 'none', marginBottom: 10 }}
                />
                {e2eePairingError && <div style={{ color: 'var(--danger)', fontSize: 12.5, marginBottom: 10 }}>{e2eePairingError}</div>}
                <div style={{ display: 'flex', gap: 10, justifyContent: 'flex-end' }}>
                  <button onClick={() => setE2eePairModal(null)}
                    style={{ background: 'transparent', border: '1px solid var(--hairline)', padding: '7px 14px', borderRadius: 'var(--r-pill)', fontSize: 12.5, cursor: 'pointer', color: 'var(--ink)' }}>
                    Cancel
                  </button>
                  <button disabled={e2eePairingLoading || !e2eePairingMnemonic.trim()}
                    onClick={async () => {
                      setE2eePairingLoading(true); setE2eePairingError(null);
                      try {
                        await e2eePair(e2eePairModal!, e2eePairingMnemonic.trim());
                        setE2eePairModal(null);
                      } catch (err) {
                        setE2eePairingError(err instanceof Error ? err.message : 'Pairing failed');
                      }
                      setE2eePairingLoading(false);
                    }}
                    style={{ background: 'var(--clay)', color: '#fff', border: 'none', padding: '7px 18px', borderRadius: 'var(--r-pill)', fontSize: 12.5, fontWeight: 500, cursor: e2eePairingLoading ? 'wait' : 'pointer', opacity: e2eePairingLoading ? 0.6 : 1 }}>
                    {e2eePairingLoading ? 'Pairing…' : 'Pair'}
                  </button>
                </div>
              </div>
            </div>
          )}

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

function PairRow({ pair, syncing, syncElapsed, lastDuration, deleting, syncDisabled, syncError, onSync, onDelete, onE2eeInit, e2eeInitLoading, onE2eePair, onE2eeDisable }: {
  pair: PairDto;
  syncing: boolean;
  syncElapsed: number;
  lastDuration: number | null;
  deleting: boolean;
  syncDisabled?: boolean;
  syncError?: string | null;
  onSync: () => void;
  onDelete: () => void;
  onE2eeInit?: () => void;
  e2eeInitLoading?: boolean;
  onE2eePair?: () => void;
  onE2eeDisable?: () => void;
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
      style={{ padding: '16px 18px', background: h ? 'var(--paper-2)' : 'var(--paper)', border: `1px solid ${syncError ? 'var(--clay)' : 'var(--hairline)'}`, borderRadius: 'var(--r-3)', marginBottom: 10, transition: 'background 0.12s, border-color 0.12s' }}>
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
            {pair.e2ee_enabled && (
              <div style={{ display: 'flex', alignItems: 'center', gap: 3, fontSize: 10, fontFamily: 'var(--mono)', background: 'var(--forest)', color: '#fff', borderRadius: 3, padding: '1px 5px', letterSpacing: '0.06em' }}>
                <Icon name="shield" size={9} color="#fff" /> E2EE
              </div>
            )}
          </div>
        </div>

        <div style={{ display: 'flex', alignItems: 'center', gap: 6, flexShrink: 0 }}>
          {/* Sync button with spinner + ETA */}
          <button onClick={onSync} disabled={syncing}
            title={syncDisabled ? 'Sync daemon is not running' : undefined}
            style={{
              background: syncing ? 'var(--cream-2)' : 'transparent',
              border: '1px solid var(--hairline)',
              padding: '6px 12px',
              borderRadius: 'var(--r-pill)',
              fontSize: 12,
              cursor: syncing ? 'not-allowed' : syncDisabled ? 'default' : 'pointer',
              opacity: syncDisabled ? 0.45 : 1,
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

          {!pair.e2ee_enabled && onE2eeInit && (
            <button onClick={onE2eeInit} disabled={e2eeInitLoading} title={e2eeInitLoading ? 'Enabling E2EE… (RSA keygen takes ~2 s)' : 'Enable E2EE'}
              style={{ background: 'transparent', border: '1px solid var(--hairline)', padding: '6px 10px', borderRadius: 'var(--r-pill)', fontSize: 12, cursor: e2eeInitLoading ? 'wait' : 'pointer', color: 'var(--forest)', display: 'flex', alignItems: 'center', gap: 4, opacity: e2eeInitLoading ? 0.6 : 1 }}>
              {e2eeInitLoading ? <SpinDot color="var(--forest)" /> : <Icon name="shield" size={13} color="var(--forest)" />}
            </button>
          )}
          {pair.e2ee_enabled && onE2eePair && (
            <button onClick={onE2eePair} title="Pair another device"
              style={{ background: 'transparent', border: '1px solid var(--hairline)', padding: '6px 10px', borderRadius: 'var(--r-pill)', fontSize: 12, cursor: 'pointer', color: 'var(--forest)', display: 'flex', alignItems: 'center' }}>
              <Icon name="shield" size={13} color="var(--forest)" />
            </button>
          )}
          {pair.e2ee_enabled && onE2eeDisable && (
            <button onClick={onE2eeDisable} title="Disable E2EE"
              style={{ background: 'transparent', border: '1px solid var(--hairline)', padding: '6px 10px', borderRadius: 'var(--r-pill)', cursor: 'pointer', color: 'var(--clay)', display: 'flex', alignItems: 'center', gap: 4, fontSize: 11 }}>
              <Icon name="shield" size={11} color="var(--clay)" />Off
            </button>
          )}
          <button onClick={onDelete} disabled={!!deleting}
            style={{ background: 'transparent', border: '1px solid var(--hairline)', padding: '6px 10px', borderRadius: 'var(--r-pill)', fontSize: 12, cursor: deleting ? 'wait' : 'pointer', color: 'var(--clay)', opacity: deleting ? 0.5 : 1, display: 'flex', alignItems: 'center', transition: 'opacity 0.12s' }}>
            <Icon name="trash" size={13} color="var(--clay)" />
          </button>
        </div>
      </div>
      {syncError && (
        <div style={{ marginTop: 10, padding: '7px 10px', background: 'var(--clay)', color: '#fff', borderRadius: 'var(--r-2)', fontSize: 12, display: 'flex', alignItems: 'center', gap: 8 }}>
          <Icon name="warn" size={13} color="#fff" />
          {syncError}
        </div>
      )}
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
