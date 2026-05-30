import React, { useEffect, useRef } from 'react';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import { MarkSlur, Icon, ChromeBtn } from './shared';

type TabValue = 'files' | 'activity';

export default function Chrome({ children, tab, onTab, onSettings, pendingConflicts, onOpenConflicts }: {
  children: React.ReactNode;
  tab?: TabValue;
  onTab?: (t: TabValue) => void;
  onSettings?: () => void;
  pendingConflicts?: number;
  onOpenConflicts?: () => void;
}) {
  const searchRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        searchRef.current?.focus();
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  return (
    <div style={{ width: '100%', height: '100%', background: 'var(--cream)', display: 'flex', flexDirection: 'column', fontFamily: 'var(--body)', color: 'var(--ink)', borderRadius: 'inherit', overflow: 'hidden' }}>
      <div
        data-tauri-drag-region
        onMouseDown={(e) => {
          if (e.button !== 0) return;
          if ((e.target as HTMLElement).closest('button, input, a, [tabindex]')) return;
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
          <div ref={searchRef} tabIndex={0}
            style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 14px', background: 'var(--cream-2)', borderRadius: 'var(--r-pill)', width: 320, color: 'var(--ink-muted)', fontSize: 13, outline: 'none' }}>
            <Icon name="search" size={14} color="var(--ink-muted)" />
            <span>Search files, people, activity…</span>
            <span style={{ marginLeft: 'auto', fontFamily: 'var(--mono)', fontSize: 10, background: 'var(--cream)', padding: '2px 5px', borderRadius: 3 }}>⌘K</span>
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
          <div style={{ width: 26, height: 26, borderRadius: 13, background: 'var(--forest)', color: 'var(--cream)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 11, fontWeight: 600, marginLeft: 4 }}>S</div>
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
