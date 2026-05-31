import React, { useEffect, useRef, useState, useCallback } from 'react';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { MarkSlur, Icon, ChromeBtn } from './shared';
import { searchFiles, FileSearchResult } from '../tauri';

type TabValue = 'files' | 'activity';

export default function Chrome({ children, tab, onTab, onSettings, pendingConflicts, onOpenConflicts, accountInitial, accountColor, accountAvatarUrl, onSearchResult }: {
  children: React.ReactNode;
  tab?: TabValue;
  onTab?: (t: TabValue) => void;
  onSettings?: () => void;
  pendingConflicts?: number;
  onOpenConflicts?: () => void;
  accountInitial?: string;
  accountColor?: string;
  accountAvatarUrl?: string;
  onSearchResult?: (result: FileSearchResult) => void;
}) {
  const inputRef = useRef<HTMLInputElement>(null);
  const containerRef = useRef<HTMLDivElement>(null);
  const [query, setQuery] = useState('');
  const [results, setResults] = useState<FileSearchResult[]>([]);
  const [open, setOpen] = useState(false);
  const [activeIdx, setActiveIdx] = useState(0);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  // ⌘K / Ctrl+K focuses the search input.
  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  // Close dropdown when clicking outside.
  useEffect(() => {
    const handler = (e: MouseEvent) => {
      if (containerRef.current && !containerRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener('mousedown', handler);
    return () => document.removeEventListener('mousedown', handler);
  }, []);

  const handleChange = useCallback((e: React.ChangeEvent<HTMLInputElement>) => {
    const q = e.target.value;
    setQuery(q);
    setActiveIdx(0);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    if (!q.trim()) { setResults([]); setOpen(false); return; }
    debounceRef.current = setTimeout(() => {
      searchFiles(q.trim(), 10)
        .then(r => { setResults(r); setOpen(r.length > 0); })
        .catch(() => {});
    }, 200);
  }, []);

  const commit = useCallback((result: FileSearchResult) => {
    onSearchResult?.(result);
    setQuery('');
    setResults([]);
    setOpen(false);
    inputRef.current?.blur();
  }, [onSearchResult]);

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    if (!open || results.length === 0) return;
    if (e.key === 'ArrowDown') { e.preventDefault(); setActiveIdx(i => Math.min(i + 1, results.length - 1)); }
    else if (e.key === 'ArrowUp') { e.preventDefault(); setActiveIdx(i => Math.max(i - 1, 0)); }
    else if (e.key === 'Enter') { e.preventDefault(); commit(results[activeIdx]); }
    else if (e.key === 'Escape') { setOpen(false); inputRef.current?.blur(); }
  }, [open, results, activeIdx, commit]);

  return (
    <div style={{ width: '100%', height: '100%', background: 'var(--cream)', display: 'flex', flexDirection: 'column', fontFamily: 'var(--body)', color: 'var(--ink)', borderRadius: 'inherit', overflow: 'hidden' }}>
      <div
        onMouseDown={(e) => {
          if (e.button !== 0) return;
          // Don't initiate window drag when the user clicks an interactive element.
          if ((e.target as HTMLElement).closest('button, input, a, select, textarea, [tabindex], [role="button"]')) return;
          getCurrentWebviewWindow().startDragging();
        }}
        style={{ height: 44, flexShrink: 0, display: 'flex', alignItems: 'center', padding: '0 14px 0 18px', borderBottom: '1px solid var(--hairline)', background: 'var(--paper)', gap: 14 }}>
        {/* left: mark + wordmark + tab toggle */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <MarkSlur size={18} fill="var(--ink)" accent="var(--clay)" />
          <span style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 16, letterSpacing: '-0.05em' }}>adagio</span>
        </div>
        {tab && (
          <div style={{ display: 'flex', gap: 0, marginLeft: 18, padding: 3, background: 'var(--cream-2)', borderRadius: 'var(--r-pill)' }}>
            {(['files', 'activity'] as TabValue[]).map((id) => (
              <button key={id} onClick={() => onTab?.(id)}
                style={{ background: tab === id ? 'var(--cream)' : 'transparent', color: tab === id ? 'var(--ink)' : 'var(--ink-muted)', border: 'none', padding: '5px 14px', borderRadius: 'var(--r-pill)', fontSize: 12.5, cursor: 'pointer', fontWeight: 500, boxShadow: tab === id ? '0 1px 2px rgba(0,0,0,0.06)' : 'none' }}>
                {id === 'files' ? 'Files' : 'Activity'}
              </button>
            ))}
          </div>
        )}
        {/* center: search bar */}
        <div style={{ flex: 1, display: 'flex', justifyContent: 'center' }}>
          <div ref={containerRef} style={{ position: 'relative', width: 320 }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 14px', background: 'var(--cream-2)', borderRadius: 'var(--r-pill)', color: 'var(--ink-muted)', fontSize: 13 }}>
              <Icon name="search" size={14} color="var(--ink-muted)" />
              <input
                ref={inputRef}
                value={query}
                onChange={handleChange}
                onKeyDown={handleKeyDown}
                onFocus={() => { if (results.length > 0) setOpen(true); }}
                placeholder="Search files…"
                style={{ flex: 1, background: 'transparent', border: 'none', outline: 'none', color: 'var(--ink)', fontSize: 13, fontFamily: 'var(--body)' }}
              />
              {!query && (
                <span style={{ marginLeft: 'auto', fontFamily: 'var(--mono)', fontSize: 10, background: 'var(--cream)', padding: '2px 5px', borderRadius: 3, flexShrink: 0 }}>⌘K</span>
              )}
            </div>
            {open && results.length > 0 && (
              <div style={{ position: 'absolute', top: 'calc(100% + 6px)', left: 0, right: 0, background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-card)', boxShadow: '0 8px 24px rgba(0,0,0,0.12)', overflow: 'hidden', zIndex: 1000 }}>
                {results.map((r, i) => (
                  <button
                    key={`${r.pair_id}:${r.path}`}
                    onMouseDown={(e) => { e.preventDefault(); commit(r); }}
                    onMouseEnter={() => setActiveIdx(i)}
                    style={{ display: 'flex', alignItems: 'center', gap: 10, width: '100%', padding: '9px 14px', background: i === activeIdx ? 'var(--cream-2)' : 'transparent', border: 'none', textAlign: 'left', cursor: 'pointer', fontSize: 13 }}
                  >
                    <Icon name="file" size={13} color="var(--ink-muted)" />
                    <div style={{ overflow: 'hidden' }}>
                      <div style={{ fontWeight: 500, color: 'var(--ink)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{r.filename}</div>
                      <div style={{ fontSize: 11, color: 'var(--ink-muted)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{r.path}</div>
                    </div>
                  </button>
                ))}
              </div>
            )}
          </div>
        </div>
        {/* right: bell, settings, avatar, window controls */}
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, color: 'var(--ink-soft)' }}>
          {(pendingConflicts ?? 0) > 0 ? (
            <button
              data-testid="conflict-badge"
              onClick={onOpenConflicts}
              style={{ display: 'flex', alignItems: 'center', gap: 5, padding: '3px 8px', background: 'var(--clay)', color: '#fff', border: 'none', borderRadius: 'var(--r-pill)', fontSize: 12, fontWeight: 600, cursor: 'pointer' }}
            >
              <Icon name="bell" size={13} color="#fff" />
              {pendingConflicts}
            </button>
          ) : (
            <ChromeBtn><Icon name="bell" size={15} /></ChromeBtn>
          )}
          <ChromeBtn onClick={onSettings}><Icon name="settings" size={15} /></ChromeBtn>
          {accountAvatarUrl
            ? <img src={accountAvatarUrl} alt={accountInitial ?? ''} style={{ width: 26, height: 26, borderRadius: 13, objectFit: 'cover', marginLeft: 4, display: 'block', flexShrink: 0 }} />
            : <div style={{ width: 26, height: 26, borderRadius: 13, background: accountColor ?? 'var(--forest)', color: 'var(--cream)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 11, fontWeight: 600, marginLeft: 4, flexShrink: 0 }}>{accountInitial ?? '?'}</div>
          }
          <div style={{ display: 'flex', alignItems: 'center', gap: 0, marginLeft: 10, paddingLeft: 10, borderLeft: '1px solid var(--hairline)' }}>
            <ChromeBtn onClick={() => getCurrentWebviewWindow().minimize()}><Icon name="min" size={11} strokeWidth={1.8} /></ChromeBtn>
            <ChromeBtn onClick={() => getCurrentWebviewWindow().toggleMaximize()}><Icon name="max" size={11} strokeWidth={1.8} /></ChromeBtn>
            <ChromeBtn onClick={() => getCurrentWebviewWindow().close()}><Icon name="close" size={11} strokeWidth={1.8} /></ChromeBtn>
          </div>
        </div>
      </div>
      {children}
    </div>
  );
}
