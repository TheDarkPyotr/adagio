import React, { useState, useEffect, useCallback } from 'react';
import Chrome from './components/Chrome';
import ConflictWizard from './components/ConflictWizard';
import Sidebar from './components/Sidebar';
import FilesScene from './components/FilesScene';
import ActivityScene from './components/ActivityScene';
import ShareDialog from './components/ShareDialog';
import OnboardingWizard from './components/OnboardingWizard';
import NewFileDialog from './components/NewFileDialog';
import SettingsScene from './components/SettingsScene';
import PairsScene from './components/PairsScene';
import TrayPopover from './components/TrayPopover';
import { listAccounts, listPairs, getStatus, getPalette, setPalette as ipcSetPalette, removeAccount, pauseSync, resumeSync, listConflicts, listenConflictDetected, listenConflictResolved, resolveConflict as ipcResolveConflict, dismissAllConflicts, listenDaemonConnectionState, startDaemon } from './tauri';
import { open as shellOpen } from '@tauri-apps/plugin-shell';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import useLocalMeta from './useLocalMeta';
import SectionScene from './components/SectionScene';
import type { AccountDto, PairDto, SyncStatusDto, ConflictDto } from './tauri';
import type { SidebarAccount } from './components/Sidebar';

const IS_TRAY = new URLSearchParams(window.location.search).has('tray');

type Section = 'all' | 'fav' | 'recent' | 'shared' | 'tags';
type View = 'main' | 'settings' | 'pairs' | 'add-account';

const DEV_FORCE_ONBOARD = false; // flip to true to test onboarding

export default function App() {
  if (IS_TRAY) return <TrayPopover />;

  const [tab, setTab] = useState<'files' | 'activity'>('files');
  const [source, setSource] = useState<Section>('all');
  const [accounts, setAccounts] = useState<AccountDto[]>([]);
  const [activeAccountId, setActiveAccountId] = useState<string | null>(null);
  const [pairs, setPairs] = useState<PairDto[]>([]);
  const [syncStatus, setSyncStatus] = useState<SyncStatusDto | null>(null);
  const [shareTarget, setShareTarget] = useState<string | null>(null);
  const [newFileOpen, setNewFileOpen] = useState(false);
  const [onboarded, setOnboarded] = useState<boolean | null>(null);
  const [view, setView] = useState<View>('main');
  const [palette, setPalette] = useState('sienna');
  const [filePath, setFilePath] = useState('/');
  const [pendingConflicts, setPendingConflicts] = useState(0);
  const [wizardOpen, setWizardOpen] = useState(false);
  const [wizardConflicts, setWizardConflicts] = useState<ConflictDto[]>([]);
  const [daemonState, setDaemonState] = useState<'connected' | 'reconnecting' | 'stopped' | 'failed' | null>(null);
  const { favorites, allTaggedFiles, pathTags, toggleFavorite, addTag, removeTag } = useLocalMeta();

  // Load accounts + pairs; called on mount and whenever the daemon connects.
  const loadInitialData = useCallback(async () => {
    if (DEV_FORCE_ONBOARD) { setOnboarded(false); return; }
    try {
      const accs = await listAccounts();
      if (accs.length) {
        setAccounts(accs);
        setActiveAccountId(prev => prev ?? accs[0].id);
        setOnboarded(true);
      } else {
        // Only go to onboarding if we haven't already loaded accounts before
        setOnboarded(o => o === null ? false : o);
      }
    } catch {
      setOnboarded(o => o === null ? true : o);
    }
    try {
      const ps = await listPairs();
      setPairs(ps);
      const allConflicts = await Promise.all(ps.map(p => listConflicts(p.id).catch(() => [] as ConflictDto[])));
      setPendingConflicts(allConflicts.flat().filter(c => !c.resolution).length);
    } catch {}
  }, []);

  // Load accounts + pairs once on mount
  useEffect(() => {
    loadInitialData();
  }, [loadInitialData]);

  // Load and apply saved palette
  useEffect(() => {
    getPalette().then(name => {
      setPalette(name);
      document.documentElement.setAttribute('data-palette', name);
    }).catch(() => {});
  }, []);

  // Subscribe to daemon connection state changes.
  // Re-load accounts/pairs whenever the daemon (re)connects so the UI
  // reflects data even if the initial load ran before the connection was ready.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listenDaemonConnectionState(({ state }) => {
      setDaemonState(state);
      if (state === 'connected') {
        // Daemon just connected (or reconnected): refresh all data.
        loadInitialData();
      }
    })
      .then(fn => { unlisten = fn; }).catch(() => {});
    return () => { unlisten?.(); };
  }, [loadInitialData]);

  // Subscribe to push conflict events to keep badge count current
  useEffect(() => {
    let unlisten1: (() => void) | undefined;
    let unlisten2: (() => void) | undefined;
    listenConflictDetected(({ pending_count }) => setPendingConflicts(pending_count))
      .then(fn => { unlisten1 = fn; }).catch(() => {});
    listenConflictResolved(({ pending_count }) => setPendingConflicts(pending_count))
      .then(fn => { unlisten2 = fn; }).catch(() => {});
    return () => { unlisten1?.(); unlisten2?.(); };
  }, []);

  // Global keyboard shortcuts (⌘O / ⌘B / ⌘P / ⌘, / ⌘Q)
  useEffect(() => {
    const handler = async (e: KeyboardEvent) => {
      if (!(e.metaKey || e.ctrlKey)) return;
      switch (e.key) {
        case 'o': {
          e.preventDefault();
          const ps = await listPairs().catch(() => []);
          if (ps[0]?.local_root) shellOpen(ps[0].local_root).catch(() => {});
          break;
        }
        case 'b': {
          e.preventDefault();
          const accs = await listAccounts().catch(() => []);
          if (accs[0]?.server_url) shellOpen(accs[0].server_url).catch(() => {});
          break;
        }
        case 'p': {
          e.preventDefault();
          getStatus().then(s => {
            if (s.status === 'paused') resumeSync().catch(() => {});
            else pauseSync().catch(() => {});
          }).catch(() => {});
          break;
        }
        case ',': {
          e.preventDefault();
          setView('settings');
          break;
        }
        case 'q': {
          e.preventDefault();
          getCurrentWebviewWindow().close().catch(() => {});
          break;
        }
      }
    };
    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, []);

  // Poll sync status every 2s
  useEffect(() => {
    const load = () => getStatus().then(setSyncStatus).catch(() => {});
    load();
    const t = setInterval(load, 2000);
    return () => clearInterval(t);
  }, []);

  const handlePalette = (name: string) => {
    setPalette(name);
    document.documentElement.setAttribute('data-palette', name);
    ipcSetPalette(name).catch(() => {});
  };

  const handleOpenConflicts = () => {
    Promise.all(pairs.map(p => listConflicts(p.id).catch(() => [] as ConflictDto[])))
      .then(all => {
        const unresolved = all.flat().filter(c => !c.resolution);
        setWizardConflicts(unresolved);
        setWizardOpen(true);
      }).catch(() => {});
  };

  const firstPair = pairs[0] ?? null;
  const activeAccount = accounts.find(a => a.id === activeAccountId) ?? accounts[0] ?? null;
  const serverHost = activeAccount ? (() => { try { return new URL(activeAccount.server_url).hostname; } catch { return activeAccount.server_url; } })() : '';

  const ACCOUNT_COLORS = ['var(--forest)', 'var(--clay)', '#2f6fcf', '#9550b8', '#b76b35', '#1c8c6e'];
  const sidebarAccounts: SidebarAccount[] = accounts.map((a, i) => ({
    id: a.id,
    name: a.display_name,
    host: (() => { try { return new URL(a.server_url).hostname; } catch { return a.server_url; } })(),
    initial: a.display_name?.trim().charAt(0).toUpperCase() ?? '?',
    color: ACCOUNT_COLORS[i % ACCOUNT_COLORS.length],
  }));

  const handleRemoveAccount = async (id: string) => {
    try { await removeAccount(id); } catch {}
    const next = accounts.filter(a => a.id !== id);
    setAccounts(next);
    if (activeAccountId === id) setActiveAccountId(next[0]?.id ?? null);
    if (next.length === 0) setOnboarded(false);
  };

  const handleAddAccountComplete = () => {
    const prevIds = new Set(accounts.map(a => a.id));
    listAccounts().then(accs => {
      setAccounts(accs);
      const newAcc = accs.find(a => !prevIds.has(a.id));
      if (newAcc) setActiveAccountId(newAcc.id);
      setView('main');
    }).catch(() => setView('main'));
  };

  // While we haven't heard from the daemon yet, show a neutral loading state.
  if (onboarded === null) {
    return (
      <div style={{ width: '100%', height: '100vh', display: 'flex', alignItems: 'center', justifyContent: 'center', background: 'var(--cream)', color: 'var(--ink-muted)', fontSize: 13 }}>
        {daemonState === 'reconnecting' || daemonState === null ? 'Connecting to sync service…' : 'Starting…'}
      </div>
    );
  }

  if (!onboarded) {
    return (
      <div style={{ width: '100%', height: '100vh', position: 'relative', overflow: 'hidden', borderRadius: 'inherit' }}>
        <Chrome>
          <OnboardingWizard onComplete={() => setOnboarded(true)} />
        </Chrome>
      </div>
    );
  }

  return (
    <div style={{ width: '100%', height: '100vh', position: 'relative', overflow: 'hidden', borderRadius: 'inherit' }}>
      <Chrome tab={view === 'main' ? tab : undefined} onTab={setTab} onSettings={() => setView('settings')} pendingConflicts={pendingConflicts} onOpenConflicts={handleOpenConflicts}>
        {view === 'add-account' ? (
          <OnboardingWizard onComplete={handleAddAccountComplete} />
        ) : view === 'settings' ? (
          <SettingsScene
            palette={palette}
            onPalette={handlePalette}
            syncStatus={syncStatus}
            onBack={() => setView('main')}
            onPairs={() => setView('pairs')}
          />
        ) : view === 'pairs' ? (
          <PairsScene
            pairs={pairs}
            account={activeAccount}
            onBack={() => setView('settings')}
            onPairsChange={setPairs}
          />
        ) : (
          <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
            <Sidebar
              selected={source}
              onSelect={setSource}
              accounts={sidebarAccounts}
              activeAccountId={activeAccountId}
              onSwitchAccount={setActiveAccountId}
              onAddAccount={() => setView('add-account')}
              onRemoveAccount={handleRemoveAccount}
              pairs={pairs}
              syncStatus={syncStatus}
            />
            {tab === 'files' ? (
              source === 'all' ? (
                <FilesScene
                  pairId={firstPair?.id ?? null}
                  serverHost={serverHost}
                  currentPath={filePath}
                  onPathChange={setFilePath}
                  onShare={setShareTarget}
                  onNew={() => setNewFileOpen(true)}
                  syncStatus={syncStatus}
                  favorites={favorites}
                  onToggleFavorite={toggleFavorite}
                />
              ) : (
                <SectionScene
                  section={source}
                  pairId={firstPair?.id ?? null}
                  favorites={favorites}
                  allTaggedFiles={allTaggedFiles}
                  pathTags={pathTags}
                  onToggleFavorite={toggleFavorite}
                  onAddTag={addTag}
                  onRemoveTag={removeTag}
                  onShare={setShareTarget}
                  onOpenFolder={path => { setFilePath(path); setSource('all'); }}
                />
              )
            ) : (
              <ActivityScene />
            )}
          </div>
        )}
      </Chrome>
      {shareTarget && <ShareDialog path={shareTarget} onClose={() => setShareTarget(null)} />}
      {newFileOpen && <NewFileDialog onClose={() => setNewFileOpen(false)} />}
      {wizardOpen && wizardConflicts.length > 0 && (
        <ConflictWizard
          conflicts={wizardConflicts}
          onClose={() => setWizardOpen(false)}
          resolveConflict={ipcResolveConflict}
          dismissAll={dismissAllConflicts}
        />
      )}
      {/* T056 — Daemon reconnection banner / error overlay */}
      {daemonState === 'reconnecting' && (
        <div data-testid="reconnecting-banner" style={{
          position: 'fixed', top: 0, left: 0, right: 0,
          background: 'var(--clay)', color: '#fff',
          padding: '8px 16px', fontSize: 13, textAlign: 'center', zIndex: 2000,
        }}>
          Reconnecting to background sync…
        </div>
      )}
      {daemonState === 'failed' && (
        <div data-testid="daemon-failed-overlay" style={{
          position: 'fixed', inset: 0,
          background: 'rgba(0,0,0,0.6)',
          display: 'flex', alignItems: 'center', justifyContent: 'center',
          zIndex: 2000,
        }}>
          <div style={{
            background: 'var(--paper)', borderRadius: 'var(--r-3)',
            padding: '28px 32px', maxWidth: 360, textAlign: 'center',
            boxShadow: '0 8px 32px rgba(0,0,0,0.2)',
          }}>
            <h2 style={{ margin: '0 0 8px', fontSize: 17 }}>Background sync stopped</h2>
            <p style={{ margin: '0 0 20px', color: 'var(--ink-muted)', fontSize: 13 }}>
              The sync process could not be restarted automatically.
            </p>
            <button
              data-testid="restart-sync-btn"
              onClick={() => { startDaemon().catch(() => {}); setDaemonState('reconnecting'); }}
              style={{
                padding: '10px 24px', background: 'var(--forest)', color: '#fff',
                border: 'none', borderRadius: 'var(--r-2)', fontSize: 14,
                fontWeight: 600, cursor: 'pointer',
              }}
            >
              Restart sync
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
