import React, { useState, useEffect, useMemo } from 'react';
import { Icon } from './shared';
import type { ActivityEntryDto } from '../tauri';
import { getActivityLog } from '../tauri';

function relativeTime(ms: number): string {
  const diff = Date.now() - ms;
  const secs = Math.floor(diff / 1000);
  if (secs < 60) return `${secs}s ago`;
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins} min ago`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs} hr ago`;
  const days = Math.floor(hrs / 24);
  if (days === 1) return 'yesterday';
  return `${days} days ago`;
}

function bucketLabel(ms: number): string {
  const diff = Date.now() - ms;
  const hrs = diff / (1000 * 60 * 60);
  if (hrs < 24) return 'Today';
  if (hrs < 48) return 'Yesterday';
  return 'Earlier this week';
}

type FilterId = 'all' | 'edit' | 'share' | 'sync' | 'conflict';

const FILTERS: [FilterId, string][] = [
  ['all', 'All'],
  ['edit', 'Edits'],
  ['share', 'Shares'],
  ['sync', 'Sync'],
  ['conflict', 'Conflicts'],
];

export default function ActivityScene() {
  const [filter, setFilter] = useState<FilterId>('all');
  const [entries, setEntries] = useState<ActivityEntryDto[]>([]);

  useEffect(() => {
    const load = () => {
      const f = filter === 'all' ? undefined : filter;
      getActivityLog(100, f).then(setEntries).catch(() => {});
    };
    load();
    const t = setInterval(load, 10000);
    return () => clearInterval(t);
  }, [filter]);

  const grouped = useMemo(() => {
    const buckets: Record<string, ActivityEntryDto[]> = {
      'Today': [],
      'Yesterday': [],
      'Earlier this week': [],
    };
    entries.forEach(a => {
      const k = bucketLabel(a.at);
      if (!buckets[k]) buckets[k] = [];
      buckets[k].push(a);
    });
    return buckets;
  }, [entries]);

  const filterCounts: Record<FilterId, number> = useMemo(() => ({
    all: entries.length,
    edit: entries.filter(e => e.kind === 'edit').length,
    share: entries.filter(e => e.kind === 'share').length,
    sync: entries.filter(e => e.kind === 'sync').length,
    conflict: entries.filter(e => e.kind === 'conflict').length,
  }), [entries]);

  return (
    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden', background: 'var(--cream)' }}>
      <div style={{ height: 52, flexShrink: 0, padding: '0 22px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', borderBottom: '1px solid var(--hairline)' }}>
        <div style={{ display: 'flex', alignItems: 'baseline', gap: 14 }}>
          <span style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 20, letterSpacing: '-0.04em' }}>Activity</span>
          <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)', letterSpacing: '0.08em' }}>since you last checked in · {entries.length} events</span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 4, background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-pill)', padding: 3 }}>
          {FILTERS.map(([id, lbl]) => (
            <button key={id} onClick={() => setFilter(id)} style={{
              background: filter === id ? 'var(--ink)' : 'transparent',
              color: filter === id ? 'var(--cream)' : 'var(--ink-soft)',
              border: 'none', padding: '5px 12px', borderRadius: 'var(--r-pill)', fontSize: 12, cursor: 'pointer', fontWeight: 500,
              display: 'flex', alignItems: 'center', gap: 6,
            }}>
              {lbl} <span style={{ fontFamily: 'var(--mono)', fontSize: 10, opacity: 0.6 }}>{filterCounts[id]}</span>
            </button>
          ))}
        </div>
      </div>

      <div style={{ flex: 1, overflowY: 'auto', padding: '8px 0 24px' }}>
        {Object.entries(grouped).map(([bucket, items]) => {
          if (items.length === 0) return null;
          return (
            <div key={bucket}>
              <div style={{ padding: '20px 28px 10px', fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.22em', textTransform: 'uppercase', color: 'var(--ink-muted)' }}>{bucket}</div>
              {items.map(a => <ActivityRow key={a.id} a={a} />)}
            </div>
          );
        })}
        {entries.length === 0 && (
          <p style={{ padding: '24px 28px', color: 'var(--ink-muted)', fontFamily: 'var(--body)', fontSize: 13 }}>No activity yet.</p>
        )}
      </div>
    </div>
  );
}

const TINT: Record<string, [string, string]> = {
  edit:     ['var(--clay)',     'edited'],
  share:    ['var(--forest)',   'shared'],
  sync:     ['var(--ink-soft)', 'synced'],
  conflict: ['var(--danger)',   'conflict'],
  pin:      ['var(--ink)',      'pinned'],
  add:      ['var(--forest)',   'added'],
  block:    ['var(--ink-muted)','blocked'],
};

function ActivityRow({ a }: { a: ActivityEntryDto }) {
  const [h, setH] = useState(false);
  const tint = TINT[a.kind] ?? ['var(--ink-muted)', a.verb];
  const [tintColor] = tint;
  const when = relativeTime(a.at);

  return (
    <div onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{ display: 'flex', alignItems: 'center', gap: 16, padding: '14px 28px',
        background: h ? 'color-mix(in srgb, var(--ink) 4%, transparent)' : 'transparent', cursor: 'pointer',
        borderBottom: '1px solid var(--hairline-2)' }}>
      <div style={{ width: 32, height: 32, borderRadius: 16, background: 'var(--paper-2)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0, color: tintColor }}>
        {a.kind === 'edit'     && <Icon name="pencil" size={14} color={tintColor} />}
        {a.kind === 'share'    && <Icon name="share"  size={14} color={tintColor} />}
        {a.kind === 'sync'     && <Icon name="sync"   size={14} color={tintColor} />}
        {a.kind === 'conflict' && <Icon name="warn"   size={14} color={tintColor} />}
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 14, color: 'var(--ink)' }}>
          <span style={{ fontWeight: 500 }}>{a.who}</span>{' '}
          <span style={{ color: 'var(--ink-muted)' }}>{a.verb}</span>{' '}
          <span style={{ fontFamily: 'var(--body)', fontWeight: 500, letterSpacing: '-0.02em', fontSize: 14, color: 'var(--ink)' }}>{a.target}</span>
          {a.with_whom && <span style={{ color: 'var(--ink-muted)' }}> · with {a.with_whom}</span>}
        </div>
        <div style={{ fontSize: 11.5, color: 'var(--ink-muted)', fontFamily: 'var(--mono)', letterSpacing: '0.03em', marginTop: 3 }}>
          {a.where_path} · {when}
        </div>
      </div>
      {h && (
        <div style={{ display: 'flex', gap: 4 }}>
          <button style={{ background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 6, padding: '5px 10px', fontSize: 11.5, cursor: 'pointer', color: 'var(--ink-soft)' }}>Open</button>
          {a.kind === 'conflict' && <button style={{ background: 'var(--danger)', color: 'var(--cream)', border: 'none', borderRadius: 6, padding: '5px 10px', fontSize: 11.5, cursor: 'pointer' }}>Resolve</button>}
        </div>
      )}
    </div>
  );
}
