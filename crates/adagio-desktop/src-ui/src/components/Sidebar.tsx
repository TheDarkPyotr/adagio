import React, { useState, useEffect, useRef } from 'react';
import { Icon, SpinDot } from './shared';
import type { PairDto, SyncStatusDto } from '../tauri';

type Section = 'all' | 'fav' | 'recent' | 'shared' | 'tags';

export interface SidebarAccount {
  id: string;
  name: string;
  host: string;
  initial: string;
  color: string;
}

export default function Sidebar({ selected, onSelect, accounts, activeAccountId, onSwitchAccount, onAddAccount, onRemoveAccount, pairs, activePairId, onSelectPair, syncStatus }: {
  selected: Section;
  onSelect: (s: Section) => void;
  accounts: SidebarAccount[];
  activeAccountId: string | null;
  onSwitchAccount: (id: string) => void;
  onAddAccount: () => void;
  onRemoveAccount: (id: string) => void;
  pairs: PairDto[];
  activePairId: string | null;
  onSelectPair: (id: string) => void;
  syncStatus: SyncStatusDto | null;
}) {
  const [dropOpen, setDropOpen] = useState(false);
  const dropRef = useRef<HTMLDivElement>(null);
  const acc = accounts.find(a => a.id === activeAccountId) ?? accounts[0] ?? { id: '', name: 'Loading…', host: '', initial: '…', color: 'var(--forest)' };

  useEffect(() => {
    if (!dropOpen) return;
    const handler = (e: MouseEvent) => {
      if (dropRef.current && !dropRef.current.contains(e.target as Node)) setDropOpen(false);
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, [dropOpen]);

  return (
    <div style={{ width: 248, flexShrink: 0, background: 'var(--paper)', borderRight: '1px solid var(--hairline)', display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
      {/* account picker */}
      <div ref={dropRef} style={{ padding: '14px 14px 12px', position: 'relative' }}>
        <button
          onClick={() => setDropOpen(o => !o)}
          style={{ width: '100%', background: dropOpen ? 'var(--cream)' : 'var(--cream-2)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', padding: 10, display: 'flex', alignItems: 'center', gap: 10, cursor: 'pointer', textAlign: 'left' }}>
          <div style={{ width: 30, height: 30, borderRadius: 8, background: acc.color, color: 'var(--cream)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontFamily: 'var(--body)', fontWeight: 600, fontSize: 14, letterSpacing: '-0.04em', flexShrink: 0 }}>{acc.initial}</div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--ink)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{acc.name}</div>
            <div style={{ fontSize: 11, color: 'var(--ink-muted)', fontFamily: 'var(--mono)', letterSpacing: '0.02em', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{acc.host}</div>
          </div>
          <div style={{ transform: dropOpen ? 'rotate(180deg)' : 'none', transition: 'transform 0.15s', flexShrink: 0 }}>
            <Icon name="caret-down" size={13} color="var(--ink-muted)" />
          </div>
        </button>

        {dropOpen && (
          <div style={{ position: 'absolute', top: 'calc(100% - 12px)', left: 14, right: 14, background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', boxShadow: '0 6px 20px rgba(0,0,0,0.10)', zIndex: 200, overflow: 'hidden' }}>
            {accounts.map(a => (
              <DropAccountRow
                key={a.id}
                account={a}
                active={a.id === acc.id}
                onSelect={() => { onSwitchAccount(a.id); setDropOpen(false); }}
                onRemove={accounts.length > 1 ? () => { onRemoveAccount(a.id); setDropOpen(false); } : undefined}
              />
            ))}
            <div style={{ borderTop: '1px solid var(--hairline)', padding: 4 }}>
              <button
                onClick={() => { onAddAccount(); setDropOpen(false); }}
                style={{ width: '100%', background: 'transparent', border: 'none', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 8, padding: '7px 10px', borderRadius: 'var(--r-1)', color: 'var(--ink-soft)', fontSize: 13 }}>
                <Icon name="plus" size={14} color="var(--ink-muted)" />
                Add account
              </button>
            </div>
          </div>
        )}
      </div>

      <SidebarSection label="Library">
        <SidebarItem icon="folder" label="All files" badge="3.2k" id="all" selected={selected} onSelect={onSelect} />
        <SidebarItem icon="star" label="Favorites" badge="14" id="fav" selected={selected} onSelect={onSelect} />
        <SidebarItem icon="clock" label="Recent" id="recent" selected={selected} onSelect={onSelect} />
        <SidebarItem icon="people" label="Shared" badge="8" id="shared" selected={selected} onSelect={onSelect} />
        <SidebarItem icon="tag" label="Tagged" id="tags" selected={selected} onSelect={onSelect} />
      </SidebarSection>

      <SidebarSection label="Pinned folders">
        {pairs.map(pair => {
          const name = pair.local_root.split('/').filter(Boolean).pop() ?? pair.local_root;
          const isSyncing = syncStatus?.status === 'syncing';
          const isActive = pair.id === activePairId;
          return (
            <PinnedFolder
              key={pair.id}
              name={name}
              status={isSyncing ? 'sync' : 'ok'}
              active={isActive}
              isVfs={pair.vfs_enabled}
              onClick={() => { onSelectPair(pair.id); onSelect('all'); }}
            />
          );
        })}
        {pairs.length === 0 && <div style={{ padding: '6px 9px', fontSize: 13, color: 'var(--ink-muted)' }}>No folders yet</div>}
      </SidebarSection>

      <div style={{ flex: 1 }}/>

      {syncStatus && syncStatus.status === 'syncing' && <SyncFooter syncStatus={syncStatus} />}
    </div>
  );
}

function DropAccountRow({ account, active, onSelect, onRemove }: {
  account: SidebarAccount;
  active: boolean;
  onSelect: () => void;
  onRemove?: () => void;
}) {
  const [hovered, setHovered] = useState(false);
  return (
    <div
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
      onClick={onSelect}
      style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '8px 10px', cursor: 'pointer', background: active ? 'var(--cream-2)' : hovered ? 'color-mix(in srgb, var(--ink) 4%, transparent)' : 'transparent', transition: 'background 0.08s' }}>
      <div style={{ width: 26, height: 26, borderRadius: 7, background: account.color, color: 'var(--cream)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontWeight: 600, fontSize: 12, flexShrink: 0 }}>{account.initial}</div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 12.5, fontWeight: 500, color: 'var(--ink)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{account.name}</div>
        <div style={{ fontSize: 10.5, color: 'var(--ink-muted)', fontFamily: 'var(--mono)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{account.host}</div>
      </div>
      {active
        ? <Icon name="check" size={13} color="var(--clay)" strokeWidth={2.5} />
        : onRemove && hovered && (
            <button
              onClick={(e) => { e.stopPropagation(); onRemove(); }}
              style={{ background: 'none', border: 'none', cursor: 'pointer', padding: 2, display: 'flex', color: 'var(--ink-muted)', borderRadius: 4 }}>
              <Icon name="trash" size={13} color="var(--ink-muted)" />
            </button>
          )
      }
    </div>
  );
}

function SidebarSection({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div style={{ padding: '4px 10px 8px' }}>
      <div style={{ padding: '6px 8px 8px', fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.16em', textTransform: 'uppercase', color: 'var(--ink-muted)' }}>{label}</div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 1 }}>{children}</div>
    </div>
  );
}

function SidebarItem({ icon, label, badge, id, selected, onSelect }: {
  icon: 'folder' | 'star' | 'clock' | 'people' | 'tag';
  label: string;
  badge?: string;
  id: Section;
  selected: Section;
  onSelect: (s: Section) => void;
}) {
  const active = selected === id;
  const [h, setH] = useState(false);
  return (
    <button onClick={() => onSelect(id)}
      onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{ background: active ? 'var(--cream-2)' : h ? 'color-mix(in srgb, var(--ink) 5%, transparent)' : 'transparent', border: 'none', borderRadius: 'var(--r-2)', padding: '7px 9px', display: 'flex', alignItems: 'center', gap: 10, fontSize: 13, color: active ? 'var(--ink)' : 'var(--ink-soft)', cursor: 'pointer', fontWeight: active ? 500 : 400, textAlign: 'left' }}>
      <Icon name={icon} size={15} color={active ? 'var(--clay)' : 'var(--ink-muted)'} />
      <span style={{ flex: 1 }}>{label}</span>
      {badge && <span style={{ fontSize: 10.5, color: 'var(--ink-muted)', fontFamily: 'var(--mono)' }}>{badge}</span>}
    </button>
  );
}

function PinnedFolder({ name, status, active, isVfs, onClick }: { name: string; status: 'ok' | 'sync'; active?: boolean; isVfs?: boolean; onClick?: () => void }) {
  const [h, setH] = useState(false);
  return (
    <div
      onClick={onClick}
      onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{ padding: '6px 9px', display: 'flex', alignItems: 'center', gap: 10, fontSize: 13, color: active ? 'var(--ink)' : 'var(--ink-soft)', cursor: 'pointer', borderRadius: 'var(--r-2)', background: active ? 'color-mix(in srgb, var(--ink) 8%, transparent)' : h ? 'color-mix(in srgb, var(--ink) 4%, transparent)' : 'transparent', margin: '0 4px' }}>
      <div style={{ width: 4, height: 4, borderRadius: 2, background: status === 'sync' ? 'var(--clay)' : 'var(--good)', flexShrink: 0 }}/>
      <span style={{ flex: 1, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis', fontWeight: active ? 500 : 400 }}>{name}</span>
      {isVfs && <span style={{ fontSize: 9, fontFamily: 'var(--mono)', background: 'var(--clay)', color: '#fff', borderRadius: 3, padding: '1px 4px', letterSpacing: '0.04em', flexShrink: 0 }}>VFS</span>}
      {status === 'sync' && <SpinDot color="var(--clay)" />}
    </div>
  );
}

function SyncFooter({ syncStatus }: { syncStatus: SyncStatusDto }) {
  const pct = syncStatus.total_bytes > 0 ? Math.round((syncStatus.transferred_bytes / syncStatus.total_bytes) * 100) : 0;
  const eta = syncStatus.eta_seconds != null ? (syncStatus.eta_seconds > 60 ? `${Math.round(syncStatus.eta_seconds / 60)}m` : `${syncStatus.eta_seconds}s`) : '';
  const transferLabel = syncStatus.total_bytes > 0
    ? `~ ${fmtBytes(syncStatus.transferred_bytes)} / ${fmtBytes(syncStatus.total_bytes)} · ${eta}`
    : `~ ${eta}`;
  return (
    <div style={{ padding: '12px 14px', borderTop: '1px solid var(--hairline)', display: 'flex', flexDirection: 'column', gap: 8 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <SpinDot color="var(--clay)" />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: 12, color: 'var(--ink)', fontWeight: 500 }}>Syncing {syncStatus.active_file_count} files</div>
          <div style={{ fontSize: 11, color: 'var(--ink-muted)', fontFamily: 'var(--mono)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{transferLabel}</div>
        </div>
      </div>
      <div style={{ height: 3, background: 'var(--cream-2)', borderRadius: 2, overflow: 'hidden' }}>
        <div style={{ width: `${pct}%`, height: '100%', background: 'var(--clay)' }}/>
      </div>
    </div>
  );
}

function fmtBytes(n: number): string {
  if (n >= 1e9) return (n / 1e9).toFixed(1) + ' GB';
  if (n >= 1e6) return (n / 1e6).toFixed(1) + ' MB';
  if (n >= 1e3) return (n / 1e3).toFixed(0) + ' KB';
  return n + ' B';
}
