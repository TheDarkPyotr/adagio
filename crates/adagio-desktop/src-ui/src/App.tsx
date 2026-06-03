import React, { useState, useEffect, useCallback } from 'react';
import { createPortal } from 'react-dom';
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
import { listAccounts, listPairs, getStatus, getPalette, setPalette as ipcSetPalette, removeAccount, pauseSync, resumeSync, listConflicts, listenConflictDetected, listenConflictResolved, resolveConflict as ipcResolveConflict, dismissAllConflicts, listenDaemonConnectionState, startDaemon, getDaemonStatus, listCustomPalettes, getAccountAvatar, getSectionCounts } from './tauri';
import type { FileSearchResult, SectionCounts } from './tauri';
import { open as shellOpen } from '@tauri-apps/plugin-shell';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import useLocalMeta from './useLocalMeta';
import SectionScene from './components/SectionScene';
import type { AccountDto, PairDto, SyncStatusDto, ConflictDto, CustomPaletteDto } from './tauri';
import type { SidebarAccount } from './components/Sidebar';
import { applyCustomPaletteTokens, clearCustomPaletteTokens } from './paletteUtils';

const IS_TRAY = new URLSearchParams(window.location.search).has('tray');

type Section = 'all' | 'fav' | 'recent' | 'shared' | 'tags';
type View = 'main' | 'settings' | 'pairs' | 'add-account';

const DEV_FORCE_ONBOARD = false; // flip to true to test onboarding

function TrayRoot() {
  const [ready, setReady] = useState(false);

  useEffect(() => {
    listCustomPalettes().then(customs => {
      getPalette().then(name => {
        if (name.startsWith('custom-')) {
          const custom = customs.find(p => p.id === name);
          if (custom) applyCustomPaletteTokens(custom);
        } else {
          document.documentElement.dataset.palette = name;
        }
        setReady(true);
      }).catch(() => setReady(true));
    }).catch(() => {
      getPalette().then(name => {
        document.documentElement.dataset.palette = name;
        setReady(true);
      }).catch(() => setReady(true));
    });
  }, []);

  if (!ready) return null;
  return <TrayPopover />;
}

export default function App() {
  if (IS_TRAY) return <TrayRoot />;

  const [tab, setTab] = useState<'files' | 'activity'>('files');
  const [source, setSource] = useState<Section>('all');
  const [accounts, setAccounts] = useState<AccountDto[]>([]);
  const [avatarUrls, setAvatarUrls] = useState<Record<string, string>>({});
  const [activeAccountId, setActiveAccountId] = useState<string | null>(null);
  const [pairs, setPairs] = useState<PairDto[]>([]);
  const [syncStatus, setSyncStatus] = useState<SyncStatusDto | null>(null);
  const [shareTarget, setShareTarget] = useState<string | null>(null);
  const [newFileOpen, setNewFileOpen] = useState(false);
  const [onboarded, setOnboarded] = useState<boolean | null>(null);
  const [view, setView] = useState<View>('main');
  const [palette, setPalette] = useState('sienna');
  const [customPalettes, setCustomPalettes] = useState<CustomPaletteDto[]>([]);
  const [filePath, setFilePath] = useState('/');
  const [highlightFile, setHighlightFile] = useState<string | null>(null);
  const [pendingConflicts, setPendingConflicts] = useState(0);
  const [wizardOpen, setWizardOpen] = useState(false);
  const [wizardConflicts, setWizardConflicts] = useState<ConflictDto[]>([]);
  const [daemonState, setDaemonState] = useState<'connected' | 'reconnecting' | 'stopped' | 'failed' | null>(null);
  const [sectionCounts, setSectionCounts] = useState<SectionCounts>({ total: 0, recent: 0 });
  const [sharedCount, setSharedCount] = useState(0);

  // Load accounts + pairs; called on mount and whenever the daemon connects.
  const loadInitialData = useCallback(async () => {
    if (DEV_FORCE_ONBOARD) { setOnboarded(false); return; }
    try {
      const accs = await listAccounts();
      if (accs.length) {
        setAccounts(accs);
        setActiveAccountId(prev => prev ?? accs[0].id);
        setOnboarded(true);
        // Fetch avatars in the background; failures fall back to initials.
        accs.forEach(a => {
          getAccountAvatar(a.id).then(url => {
            if (url) setAvatarUrls(prev => ({ ...prev, [a.id]: url }));
          }).catch(() => {});
        });
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

  // Load custom palettes and apply the saved palette on mount.
  useEffect(() => {
    // Load custom palettes first so we can apply them if selected.
    listCustomPalettes().then(customs => {
      setCustomPalettes(customs);
      // Then load and apply the stored palette name.
      getPalette().then(name => {
        setPalette(name);
        if (name.startsWith('custom-')) {
          const custom = customs.find(p => p.id === name);
          if (custom) applyCustomPaletteTokens(custom);
        } else {
          document.documentElement.dataset.palette = name;
        }
      }).catch(() => {});
    }).catch(() => {
      getPalette().then(name => {
        setPalette(name);
        document.documentElement.dataset.palette = name;
      }).catch(() => {});
    });
  }, []);

  // Poll daemon status every 3 s — primary source of truth for banners.
  // Events (see below) give sub-second updates; the poll catches anything missed.
  useEffect(() => {
    const poll = () =>
      getDaemonStatus()
        .then(s => {
          // When state_tx is stuck at 'failed' but the daemon came back on its
          // own (e.g. systemd restart), a passive status read still returns
          // 'failed' because no one called upgrade_connection yet.  Kick
          // start_daemon so it re-opens the socket and updates state_tx.
          if (s.connection_state === 'failed') {
            startDaemon().catch(() => {});
          }
          setDaemonState(s.connection_state);
        })
        .catch(() => {});
    poll();
    const id = setInterval(poll, 3000);
    return () => clearInterval(id);
  }, []);

  // Subscribe to daemon connection state changes.
  // Re-load accounts/pairs whenever the daemon (re)connects so the UI
  // reflects data even if the initial load ran before the connection was ready.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    listenDaemonConnectionState(({ state }) => {
      setDaemonState(state);
      if (state === 'connected') loadInitialData();
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

  // Navigate to settings when the tray Preferences action triggers this event.
  useEffect(() => {
    let unlisten: (() => void) | undefined;
    getCurrentWebviewWindow().listen('adagio://open-settings', () => setView('settings'))
      .then(fn => { unlisten = fn; }).catch(() => {});
    return () => { unlisten?.(); };
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
    if (name.startsWith('custom-')) {
      const custom = customPalettes.find(p => p.id === name);
      if (custom) applyCustomPaletteTokens(custom);
    } else {
      clearCustomPaletteTokens(name);
    }
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

  const [activePairId, setActivePairId] = useState<string | null>(null);

  // Favorites and tags are scoped per pair so switching accounts shows the
  // correct set. useLocalMeta reloads from localStorage whenever activePairId changes.
  const { favorites, allTaggedFiles, pathTags, toggleFavorite, addTag, removeTag } = useLocalMeta(activePairId);

  // When the active account changes, reset to the first pair of that account
  // and clear stale section counts so the sidebar doesn't flash old numbers.
  React.useEffect(() => {
    if (!activeAccountId) return;
    const firstPair = pairs.find(p => p.account_id === activeAccountId) ?? null;
    setActivePairId(firstPair?.id ?? null);
    setFilePath('/');
    setHighlightFile(null);
    setSectionCounts({ total: 0, recent: 0 });
    setSharedCount(0);
    setSource('all');
    // Notify the tray window so it reloads files for the new active account.
    import('@tauri-apps/api/webviewWindow').then(({ getAllWebviewWindows }) => {
      getAllWebviewWindows().then(wins => {
        const tray = wins.find(w => w.label === 'tray');
        tray?.emit('adagio://active-account-changed', {
          accountId: activeAccountId,
          pairId: firstPair?.id ?? null,
        }).catch(() => {});
      }).catch(() => {});
    }).catch(() => {});
  }, [activeAccountId]); // eslint-disable-line react-hooks/exhaustive-deps

  // activePair must belong to the active account; never leak a pair from another.
  const accountPairs = pairs.filter(p => p.account_id === (activeAccountId ?? accounts[0]?.id));
  const activePair = accountPairs.find(p => p.id === activePairId) ?? accountPairs[0] ?? null;

  // Fetch sidebar counts whenever the active pair changes, then every 30 s.
  React.useEffect(() => {
    let cancelled = false;
    const fetch = () => getSectionCounts(activePair?.id).then(c => { if (!cancelled) setSectionCounts(c); }).catch(() => {});
    fetch();
    const t = setInterval(fetch, 30_000);
    return () => { cancelled = true; clearInterval(t); };
  }, [activePair?.id]);

  const handleSelectPair = useCallback((id: string) => {
    setActivePairId(id);
    setFilePath('/');
    setHighlightFile(null);
  }, []);
  const activeAccount = accounts.find(a => a.id === activeAccountId) ?? accounts[0] ?? null;
  const serverHost = activeAccount ? (() => { try { return new URL(activeAccount.server_url).hostname; } catch { return activeAccount.server_url; } })() : '';

  const ACCOUNT_COLORS = ['var(--forest)', 'var(--clay)', '#2f6fcf', '#9550b8', '#b76b35', '#1c8c6e'];
  const sidebarAccounts: SidebarAccount[] = accounts.map((a, i) => ({
    id: a.id,
    name: a.display_name,
    host: (() => { try { return new URL(a.server_url).hostname; } catch { return a.server_url; } })(),
    initial: a.display_name?.trim().charAt(0).toUpperCase() ?? '?',
    color: ACCOUNT_COLORS[i % ACCOUNT_COLORS.length],
    avatar_url: avatarUrls[a.id],
  }));

  const handleRemoveAccount = async (id: string) => {
    try { await removeAccount(id); } catch {}
    const next = accounts.filter(a => a.id !== id);
    setAccounts(next);
    if (activeAccountId === id) setActiveAccountId(next[0]?.id ?? null);
    if (next.length === 0) setOnboarded(false);
  };

  // Called when onboarding or add-account wizard finishes successfully.
  // Refreshes both accounts and pairs so the sidebar reflects the new pair immediately.
  const handleOnboardingComplete = useCallback(async () => {
    try {
      const [accs, ps] = await Promise.all([listAccounts(), listPairs()]);
      setAccounts(accs);
      setPairs(ps);
      setActiveAccountId(prev => prev ?? accs[0]?.id ?? null);
    } catch {}
    setOnboarded(true);
  }, []);

  const handleAddAccountComplete = () => {
    const prevIds = new Set(accounts.map(a => a.id));
    Promise.all([listAccounts(), listPairs()]).then(([accs, ps]) => {
      setAccounts(accs);
      setPairs(ps);
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
          <OnboardingWizard onComplete={handleOnboardingComplete} />
        </Chrome>
      </div>
    );
  }

  return (
    <div style={{ width: '100%', height: '100vh', position: 'relative', overflow: 'hidden', borderRadius: 'inherit' }}>
      <Chrome tab={view === 'main' ? tab : undefined} onTab={setTab} onSettings={() => setView('settings')} pendingConflicts={pendingConflicts} onOpenConflicts={handleOpenConflicts}
        accountInitial={sidebarAccounts.find(a => a.id === (activeAccountId ?? sidebarAccounts[0]?.id))?.initial ?? '?'}
        accountColor={sidebarAccounts.find(a => a.id === (activeAccountId ?? sidebarAccounts[0]?.id))?.color ?? 'var(--forest)'}
        accountAvatarUrl={sidebarAccounts.find(a => a.id === (activeAccountId ?? sidebarAccounts[0]?.id))?.avatar_url}
        onSearchResult={(r: FileSearchResult) => {
          // Navigate to the pair, open the file's parent directory, and highlight the file.
          const parts = r.path.split('/').filter(Boolean);
          const parentPath = parts.length > 1 ? '/' + parts.slice(0, -1).join('/') : '/';
          setActivePairId(r.pair_id);
          setFilePath(parentPath);
          setHighlightFile(r.filename);
          setSource('all');
          setView('main');
          setTab('files');
        }}
      >
        {view === 'add-account' ? (
          <OnboardingWizard onComplete={() => handleAddAccountComplete()} />
        ) : view === 'settings' ? (
          <SettingsScene
            palette={palette}
            onPalette={handlePalette}
            syncStatus={syncStatus}
            onBack={() => setView('main')}
            onPairs={() => setView('pairs')}
            customPalettes={customPalettes}
            onCustomPalettesChange={setCustomPalettes}
          />
        ) : view === 'pairs' ? (
          <PairsScene
            pairs={accountPairs}
            account={activeAccount}
            onBack={() => setView('settings')}
            onPairsChange={setPairs}
            daemonState={daemonState}
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
              pairs={accountPairs}
              activePairId={activePair?.id ?? null}
              onSelectPair={handleSelectPair}
              syncStatus={syncStatus}
              totalFiles={sectionCounts.total}
              recentFiles={sectionCounts.recent}
              favoriteFiles={favorites.size}
              sharedFiles={sharedCount}
              taggedFiles={allTaggedFiles.size}
            />
            {tab === 'files' ? (
              source === 'all' ? (
                <FilesScene
                  pairId={activePair?.id ?? null}
                  isVfsPair={activePair?.vfs_enabled ?? false}
                  isE2eePair={activePair?.e2ee_enabled ?? false}
                  localRoot={activePair?.local_root}
                  serverHost={serverHost}
                  currentPath={filePath}
                  onPathChange={(p) => { setFilePath(p); setHighlightFile(null); }}
                  highlightFile={highlightFile}
                  onShare={setShareTarget}
                  onNew={() => setNewFileOpen(true)}
                  syncStatus={syncStatus}
                  favorites={favorites}
                  onToggleFavorite={toggleFavorite}
                />
              ) : (
                <SectionScene
                  section={source}
                  pairId={activePair?.id ?? null}
                  favorites={favorites}
                  allTaggedFiles={allTaggedFiles}
                  pathTags={pathTags}
                  onToggleFavorite={toggleFavorite}
                  onAddTag={addTag}
                  onRemoveTag={removeTag}
                  onShare={setShareTarget}
                  onOpenFolder={path => { setFilePath(path); setSource('all'); }}
                  onSharedCount={setSharedCount}
                />
              )
            ) : (
              <ActivityScene />
            )}
          </div>
        )}
      </Chrome>
      {shareTarget && <ShareDialog path={shareTarget} onClose={() => setShareTarget(null)} />}
      {newFileOpen && (
        <NewFileDialog
          onClose={() => setNewFileOpen(false)}
          onCreated={() => setNewFileOpen(false)}
          localRoot={activePair?.local_root ?? ''}
          currentPath={filePath}
        />
      )}
      {wizardOpen && wizardConflicts.length > 0 && (
        <ConflictWizard
          conflicts={wizardConflicts}
          onClose={() => setWizardOpen(false)}
          resolveConflict={ipcResolveConflict}
          dismissAll={dismissAllConflicts}
        />
      )}
      {/* T056 — Daemon reconnection banner / error overlay.
          Rendered via Portal into document.body to escape any CSS overflow/
          containment on ancestor divs that would clip position:fixed children. */}
      {daemonState !== 'connected' && daemonState !== null && createPortal(
        <>
          {daemonState === 'stopped' && (
            <div data-testid="daemon-stopped-banner" style={{
              position: 'fixed', top: 44, left: 0, right: 0,
              background: 'var(--clay)', color: '#fff',
              padding: '8px 20px', fontSize: 13, zIndex: 2000,
              display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 16,
            }}>
              <span>Sync is not running — your files are not being updated.</span>
              <button
                data-testid="start-sync-btn"
                onClick={() => { startDaemon().catch(() => {}); setDaemonState('reconnecting'); }}
                style={{
                  background: '#fff', color: 'var(--clay)', border: 'none',
                  borderRadius: 'var(--r-1)', padding: '4px 12px',
                  fontSize: 12, fontWeight: 600, cursor: 'pointer', flexShrink: 0,
                }}
              >
                Start sync
              </button>
            </div>
          )}
          {daemonState === 'reconnecting' && (
            <div data-testid="reconnecting-banner" style={{
              position: 'fixed', top: 44, left: 0, right: 0,
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
        </>,
        document.body,
      )}
    </div>
  );
}
