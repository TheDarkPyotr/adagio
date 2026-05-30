import React, { useState } from 'react';
import { Icon } from './shared';
import type { PairDto, AccountDto } from '../tauri';
import { createPair, deletePair, triggerSync } from '../tauri';

export default function PairsScene({ pairs, account, onBack, onPairsChange }: {
  pairs: PairDto[];
  account: AccountDto | null;
  onBack: () => void;
  onPairsChange: (pairs: PairDto[]) => void;
}) {
  const [showAdd, setShowAdd] = useState(false);
  const [localRoot, setLocalRoot] = useState('');
  const [remoteRoot, setRemoteRoot] = useState('/');
  const [adding, setAdding] = useState(false);
  const [syncing, setSyncing] = useState<string | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [addError, setAddError] = useState<string | null>(null);

  const handleAdd = async () => {
    if (!account || !localRoot.trim()) return;
    setAdding(true);
    setAddError(null);
    try {
      const pair = await createPair({ account_id: account.id, local_root: localRoot.trim(), remote_root: remoteRoot.trim() || '/' });
      onPairsChange([...pairs, pair]);
      setShowAdd(false);
      setLocalRoot('');
      setRemoteRoot('/');
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
    setSyncing(id);
    try { await triggerSync(id); } catch {}
    setSyncing(null);
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
              syncing={syncing === pair.id}
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
              {addError && (
                <div style={{ fontSize: 12, color: 'var(--clay)', marginTop: 10 }}>{addError}</div>
              )}
              <div style={{ display: 'flex', gap: 8, marginTop: 16, justifyContent: 'flex-end' }}>
                <button onClick={() => { setShowAdd(false); setAddError(null); }}
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

function PairRow({ pair, syncing, deleting, onSync, onDelete }: {
  pair: PairDto;
  syncing: boolean;
  deleting: boolean;
  onSync: () => void;
  onDelete: () => void;
}) {
  const [h, setH] = useState(false);
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
          <div style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--ink-muted)', letterSpacing: '0.08em' }}>
            {pair.id.slice(0, 8)}
          </div>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, flexShrink: 0 }}>
          <button onClick={onSync} disabled={syncing}
            style={{ background: 'transparent', border: '1px solid var(--hairline)', padding: '6px 12px', borderRadius: 'var(--r-pill)', fontSize: 12, cursor: syncing ? 'wait' : 'pointer', color: 'var(--ink-soft)', fontWeight: 500, opacity: syncing ? 0.5 : 1, display: 'flex', alignItems: 'center', gap: 5, transition: 'opacity 0.12s' }}>
            <Icon name="refresh" size={12} color="var(--ink-muted)" />
            {syncing ? 'Syncing…' : 'Sync now'}
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
