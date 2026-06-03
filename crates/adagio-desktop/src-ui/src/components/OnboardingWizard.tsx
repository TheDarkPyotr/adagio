import React, { useState, useEffect } from 'react';
import { MarkSlur, Icon, SpinDot } from './shared';
import {
  probeServer, beginAuthFlow, pickFolder, getAccountRemoteStats, completeOnboarding,
  listenAuthFlowComplete, listenAuthFlowExpired, getStatus, resumeSync,
} from '../tauri';
import type { ServerProbeDto, AuthFlowInitDto, RemoteStatsDto, SyncStatusDto } from '../tauri';

// ── Utilities ─────────────────────────────────────────────────────────────────

function formatBytes(bytes: number): string {
  if (bytes >= 1e12) return `${(bytes / 1e12).toFixed(1)} TB`;
  if (bytes >= 1e9)  return `${(bytes / 1e9).toFixed(1)} GB`;
  if (bytes >= 1e6)  return `${(bytes / 1e6).toFixed(1)} MB`;
  return `${(bytes / 1e3).toFixed(1)} KB`;
}

function hostname(url: string): string {
  try { return new URL(url).hostname; } catch { return url; }
}

// ── Sub-components ────────────────────────────────────────────────────────────

function AdagioWord({ size = 32, color = 'var(--ink)', weight = 500 }: {
  size?: number; color?: string; weight?: number;
}) {
  return (
    <span style={{ fontFamily: 'var(--body)', fontWeight: weight, fontSize: size, lineHeight: 1, letterSpacing: '-0.05em', color }}>
      adagio
    </span>
  );
}

function ToggleRow({ title, desc, value, onChange }: {
  title: string; desc: string; value: boolean; onChange: (v: boolean) => void;
}) {
  return (
    <button
      onClick={() => onChange(!value)}
      data-testid={`toggle-${title.toLowerCase().replace(/\s+/g, '-')}`}
      style={{ background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', padding: 14, textAlign: 'left', cursor: 'pointer', display: 'flex', flexDirection: 'column', gap: 4 }}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <span style={{ fontSize: 13.5, fontWeight: 500, color: 'var(--ink)' }}>{title}</span>
        <span style={{ width: 28, height: 16, background: value ? 'var(--forest)' : 'var(--cream-3)', borderRadius: 8, padding: 2, display: 'flex', justifyContent: value ? 'flex-end' : 'flex-start', transition: 'all .15s' }}>
          <span style={{ width: 12, height: 12, borderRadius: 6, background: 'var(--cream)', display: 'block' }}/>
        </span>
      </div>
      <span style={{ fontSize: 12, color: 'var(--ink-muted)', lineHeight: 1.45 }}>{desc}</span>
    </button>
  );
}

function OnboardFooter({
  step, setStep, finish, continueDisabled, continueLabel,
}: {
  step: number;
  setStep: (n: number) => void;
  finish?: boolean;
  continueDisabled?: boolean;
  continueLabel?: string;
}) {
  const label = continueLabel ?? (finish ? 'Open Adagio' : 'Continue');
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 40, paddingTop: 20, borderTop: '1px solid var(--hairline)' }}>
      <button
        onClick={() => setStep(Math.max(1, step - 1))}
        style={{ background: 'transparent', border: 'none', color: 'var(--ink-muted)', cursor: 'pointer', fontSize: 13, display: 'flex', alignItems: 'center', gap: 6 }}
      >
        ← Back
      </button>
      <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
        <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)', letterSpacing: '0.06em' }}>step {step} of 5</span>
        <button
          disabled={continueDisabled}
          onClick={() => setStep(finish ? step + 1 : Math.min(5, step + 1))}
          style={{ background: 'var(--ink)', color: 'var(--cream)', border: 'none', padding: '10px 22px', borderRadius: 'var(--r-pill)', fontSize: 13, fontWeight: 500, cursor: continueDisabled ? 'not-allowed' : 'pointer', opacity: continueDisabled ? 0.4 : 1, display: 'flex', alignItems: 'center', gap: 8 }}
        >
          {label} <Icon name="arrow-r" size={14} color="var(--cream)" />
        </button>
      </div>
    </div>
  );
}

const STEPS = [
  { n: 1, name: 'Welcome' },
  { n: 2, name: 'Server' },
  { n: 3, name: 'Authorize' },
  { n: 4, name: 'Where to sync' },
  { n: 5, name: 'Begin' },
];

const MONO_LABEL: React.CSSProperties = {
  fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.16em',
  textTransform: 'uppercase', color: 'var(--ink-muted)',
};

// ── Main wizard ───────────────────────────────────────────────────────────────

export default function OnboardingWizard({ onComplete }: { onComplete: () => void }) {
  // Navigation
  const [step, setStep] = useState(1);

  // Step 2 — server
  const [url, setUrl]     = useState('https://cloud.sound.studio');
  const [probe, setProbe] = useState<ServerProbeDto | null>(null);
  const [probing, setProbing] = useState(false);

  // Step 3 — auth flow
  const [authFlow, setAuthFlow]   = useState<AuthFlowInitDto | null>(null);
  const [authError, setAuthError] = useState<string | null>(null);
  const [authExpired, setAuthExpired] = useState(false);
  const [accountId, setAccountId] = useState<string | null>(null);
  const [countdown, setCountdown] = useState(0);

  // Step 4 — folder & prefs (lifted so they survive Back navigation)
  const [folder, setFolder]               = useState('~/Adagio');
  const [vfsEnabled, setVfsEnabled]       = useState(true);
  const [pinPinned, setPinPinned]         = useState(true);
  const [smartBandwidth, setSmartBandwidth] = useState(true);
  const [watchExternal, setWatchExternal] = useState(false);

  // Step 5 — begin
  const [remoteStats, setRemoteStats]         = useState<RemoteStatsDto | null>(null);
  const [remoteStatsFailed, setRemoteStatsFailed] = useState(false);
  const [pairError, setPairError]             = useState<string | null>(null);
  const [pairCreating, setPairCreating]       = useState(false);
  const [pairId, setPairId]                   = useState<string | null>(null);
  const [syncStatus, setSyncStatus]           = useState<SyncStatusDto | null>(null);

  // ── Effects ────────────────────────────────────────────────────────────────

  // Step 2: debounced server probe (600 ms after URL change)
  useEffect(() => {
    if (step !== 2 || !url.trim()) return;
    setProbing(true);
    const t = setTimeout(() => {
      probeServer(url)
        .then(p => { setProbe(p); setProbing(false); })
        .catch(() => { setProbe(null); setProbing(false); });
    }, 600);
    return () => clearTimeout(t);
  }, [url, step]);

  // Step 3: start Login Flow v2 session when the step becomes active
  useEffect(() => {
    if (step !== 3) return;
    let live = true;
    setAuthFlow(null);
    setAuthError(null);
    setAuthExpired(false);
    beginAuthFlow(url)
      .then(dto  => { if (live) setAuthFlow(dto); })
      .catch(e   => { if (live) setAuthError(String(e)); });
    return () => { live = false; };
  }, [step]); // url is captured at the time step 3 is entered; intentional

  // Step 3: subscribe to auth-flow events
  useEffect(() => {
    if (step !== 3) return;
    let unlistenComplete: (() => void) | null = null;
    let unlistenExpired:  (() => void) | null = null;
    listenAuthFlowComplete(payload => {
      setAccountId(payload.account.id);
      setStep(4);
    }).then(fn => { unlistenComplete = fn; });
    listenAuthFlowExpired(() => {
      setAuthExpired(true);
    }).then(fn => { unlistenExpired = fn; });
    return () => {
      unlistenComplete?.();
      unlistenExpired?.();
    };
  }, [step]);

  // Step 3: countdown timer driven by expires_at
  useEffect(() => {
    if (step !== 3 || !authFlow) return;
    const tick = () => {
      const remaining = Math.max(0, authFlow.expires_at - Math.floor(Date.now() / 1000));
      setCountdown(remaining);
      return remaining;
    };
    tick();
    const t = setInterval(() => { if (tick() === 0) clearInterval(t); }, 1000);
    return () => clearInterval(t);
  }, [step, authFlow]);

  // Step 5: create pair immediately on entry (so sync starts before the user
  // clicks "Open Adagio"), then fetch remote stats in parallel.
  useEffect(() => {
    if (step !== 5 || !accountId) return;
    let live = true;
    setPairError(null);
    setRemoteStatsFailed(false);

    // Create the pair unless already created (e.g., user backed up then returned).
    if (!pairId) {
      setPairCreating(true);
      completeOnboarding(accountId, {
        local_folder: folder,
        vfs_enabled: vfsEnabled,
        pin_pinned_folders: pinPinned,
        smart_bandwidth: smartBandwidth,
        watch_external_edits: watchExternal,
      })
        .then(async pair => {
          if (!live) return;
          setPairId(pair.id ?? null);
          setPairCreating(false);
          // Ensure sync is running — engine may be paused from a prior session.
          const s = await getStatus().catch(() => null);
          if (s?.status === 'paused') resumeSync().catch(() => {});
        })
        .catch(e => {
          if (!live) return;
          setPairError(String(e));
          setPairCreating(false);
        });
    }

    // Fetch remote quota stats in parallel.
    getAccountRemoteStats(accountId)
      .then(stats => { if (live) setRemoteStats(stats); })
      .catch(() => { if (live) setRemoteStatsFailed(true); });

    return () => { live = false; };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [step, accountId]); // intentionally omit prefs: pair is created once on entry

  // Step 5: poll sync status every 2 s once the pair is created.
  useEffect(() => {
    if (step !== 5 || !pairId) return;
    const load = () => getStatus().then(setSyncStatus).catch(() => {});
    load();
    const t = setInterval(load, 2000);
    return () => clearInterval(t);
  }, [step, pairId]);

  // ── Handlers ───────────────────────────────────────────────────────────────

  const handlePickFolder = async () => {
    const path = await pickFolder().catch(() => null);
    if (path) setFolder(path);
  };

  const handleTryAgain = () => {
    let live = true;
    setAuthExpired(false);
    setAuthFlow(null);
    setAuthError(null);
    beginAuthFlow(url)
      .then(dto => { if (live) setAuthFlow(dto); })
      .catch(e  => { if (live) setAuthError(String(e)); });
    // Note: live is not cleaned up here because handleTryAgain has no cleanup.
    // The step-3 effect will clean up if the user navigates away.
  };

  const handleFinish = () => {
    // Pair was already created when step 5 was entered; just close the wizard.
    onComplete();
  };

  const goStep = (n: number) => {
    if (n > 5) { handleFinish(); return; }
    setStep(n);
  };

  const continueDisabledStep2 = probe !== null && (!probe.reachable || !probe.version_ok);

  // ── Render ─────────────────────────────────────────────────────────────────

  return (
    <div style={{ flex: 1, display: 'flex', background: 'var(--cream)' }}>
      {/* left rail */}
      <div style={{ width: 320, padding: '60px 36px', borderRight: '1px solid var(--hairline)', display: 'flex', flexDirection: 'column' }}>
        <div style={{ fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.22em', textTransform: 'uppercase', color: 'var(--clay)', marginBottom: 14 }}>First-run · v1.0</div>
        <h2 style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 34, lineHeight: 1.08, margin: 0, letterSpacing: '-0.04em' }}>
          Let's bring your files <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontWeight: 400, letterSpacing: '-0.01em' }}>into tempo.</span>
        </h2>
        <p style={{ fontSize: 14, lineHeight: 1.6, color: 'var(--ink-soft)', marginTop: 24, marginBottom: 36 }}>
          Adagio works with any Nextcloud instance. We'll connect, authorize, and pick a quiet corner of your disk to live in.
        </p>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 0, marginTop: 'auto' }}>
          {STEPS.map(s => {
            const done = step > s.n, active = step === s.n;
            return (
              <button key={s.n} onClick={() => setStep(s.n)}
                style={{ background: 'transparent', border: 'none', textAlign: 'left', cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 14, padding: '10px 0' }}>
                <div style={{ width: 22, height: 22, borderRadius: 11, display: 'flex', alignItems: 'center', justifyContent: 'center', background: done ? 'var(--forest)' : active ? 'var(--clay)' : 'var(--cream-2)', color: done || active ? 'var(--cream)' : 'var(--ink-muted)', fontFamily: 'var(--mono)', fontSize: 11, fontWeight: 600, flexShrink: 0 }}>
                  {done ? <Icon name="check" size={10} color="var(--cream)" strokeWidth={3}/> : s.n}
                </div>
                <span style={{ fontSize: 13.5, color: active ? 'var(--ink)' : 'var(--ink-muted)', fontWeight: active ? 500 : 400 }}>{s.name}</span>
              </button>
            );
          })}
        </div>
      </div>

      {/* right content */}
      <div style={{ flex: 1, padding: '60px 80px', display: 'flex', flexDirection: 'column', justifyContent: 'space-between', overflow: 'auto' }}>

        {/* ── Step 1: Welcome ─────────────────────────────────────────────── */}
        {step === 1 && (
          <>
            <div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 14, marginBottom: 28 }}>
                <MarkSlur size={48} fill="var(--ink)" accent="var(--clay)" />
                <AdagioWord size={56} />
              </div>
              <h1 style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 56, lineHeight: 1, margin: 0, letterSpacing: '-0.05em', maxWidth: 600 }}>Welcome.</h1>
              <p style={{ marginTop: 22, fontSize: 16, lineHeight: 1.6, color: 'var(--ink-soft)', maxWidth: 520 }}>
                Adagio is a desktop client for Nextcloud. It runs quietly in the background, keeps a single folder on your machine in sync, and stays out of your way the rest of the time.
              </p>
              <div style={{ marginTop: 32, display: 'flex', flexDirection: 'column', gap: 14 }}>
                {([
                  ['No telemetry, ever.', 'Adagio does not phone home. Crash logs stay on this machine until you choose to send them.'],
                  ['Files live in your filesystem.', 'No proprietary container, no virtual mount. Open them with anything.'],
                  ['Source open under AGPL-3.0.', 'You can read every line that touches your data.'],
                ] as [string, string][]).map(([h, b]) => (
                  <div key={h} style={{ display: 'flex', gap: 14, alignItems: 'flex-start' }}>
                    <Icon name="check-circ" size={20} color="var(--forest)" />
                    <div>
                      <div style={{ fontSize: 14, fontWeight: 500 }}>{h}</div>
                      <div style={{ fontSize: 13.5, color: 'var(--ink-soft)', marginTop: 2 }}>{b}</div>
                    </div>
                  </div>
                ))}
              </div>
            </div>
            <OnboardFooter step={step} setStep={goStep} />
          </>
        )}

        {/* ── Step 2: Server ──────────────────────────────────────────────── */}
        {step === 2 && (
          <>
            <div>
              <h1 style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 52, lineHeight: 1.02, margin: 0, letterSpacing: '-0.05em' }}>
                Where does your <br/>Nextcloud <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontWeight: 400, letterSpacing: '-0.01em' }}>live?</span>
              </h1>
              <p style={{ marginTop: 20, fontSize: 15, lineHeight: 1.6, color: 'var(--ink-soft)', maxWidth: 460 }}>
                Paste your server URL. We'll auto-detect the auth flow.
              </p>
              <div style={{ marginTop: 36, maxWidth: 540 }}>
                <label style={MONO_LABEL}>Server</label>
                <div style={{ marginTop: 8, display: 'flex', alignItems: 'center', gap: 0, background: 'var(--paper)', border: `1.5px solid ${probe && !probe.reachable ? 'var(--clay)' : 'var(--ink)'}`, borderRadius: 'var(--r-2)', padding: '12px 14px' }}>
                  <Icon name="globe" size={16} color="var(--ink-muted)" />
                  <input
                    value={url}
                    onChange={e => { setUrl(e.target.value); setProbe(null); }}
                    style={{ flex: 1, border: 'none', background: 'transparent', outline: 'none', fontFamily: 'var(--mono)', fontSize: 14, color: 'var(--ink)', marginLeft: 10 }}
                  />
                  {probing && <SpinDot color="var(--clay)" />}
                  {!probing && probe?.reachable && (
                    <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--good)', display: 'flex', alignItems: 'center', gap: 6, whiteSpace: 'nowrap' }}>
                      <span style={{ width: 6, height: 6, borderRadius: 3, background: 'var(--good)' }}/>
                      Nextcloud {probe.version}
                    </span>
                  )}
                  {!probing && probe && !probe.reachable && (
                    <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--clay)', display: 'flex', alignItems: 'center', gap: 6, whiteSpace: 'nowrap' }}>
                      <span style={{ width: 6, height: 6, borderRadius: 3, background: 'var(--clay)' }}/>
                      Unreachable
                    </span>
                  )}
                </div>

                {/* Error message */}
                {probe?.error && (
                  <p style={{ margin: '8px 0 0', fontSize: 12, color: 'var(--clay)', fontFamily: 'var(--mono)' }}>
                    {probe.error}
                  </p>
                )}

                {/* Detected capabilities panel */}
                {probe?.reachable && (
                  <div style={{ marginTop: 12, padding: 14, background: 'var(--paper-2)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)' }}>
                    <div style={{ fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.16em', textTransform: 'uppercase', color: 'var(--ink-muted)', marginBottom: 10 }}>Detected</div>
                    <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10, fontSize: 13 }}>
                      <div>
                        <span style={{ color: 'var(--ink-muted)' }}>Auth flow</span><br/>
                        <span style={{ fontFamily: 'var(--mono)' }}>Login Flow v2</span>
                      </div>
                      <div>
                        <span style={{ color: 'var(--ink-muted)' }}>TLS</span><br/>
                        <span style={{ fontFamily: 'var(--mono)', color: probe.tls_valid ? 'var(--good)' : 'var(--clay)' }}>
                          {probe.tls_valid ? 'valid' : 'invalid'}
                        </span>
                      </div>
                      <div>
                        <span style={{ color: 'var(--ink-muted)' }}>E2EE</span><br/>
                        <span style={{ fontFamily: 'var(--mono)' }}>
                          {probe.e2ee_available ? 'available · per-folder' : 'not available'}
                        </span>
                      </div>
                      <div>
                        <span style={{ color: 'var(--ink-muted)' }}>Latency</span><br/>
                        <span style={{ fontFamily: 'var(--mono)' }}>{probe.latency_ms} ms</span>
                      </div>
                    </div>
                  </div>
                )}
              </div>
            </div>
            <OnboardFooter step={step} setStep={goStep} continueDisabled={continueDisabledStep2} />
          </>
        )}

        {/* ── Step 3: Authorize ───────────────────────────────────────────── */}
        {step === 3 && (
          <>
            <div>
              <h1 style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 52, lineHeight: 1.02, margin: 0, letterSpacing: '-0.05em' }}>
                Authorize Adagio <br/>in your browser.
              </h1>
              <p style={{ marginTop: 20, fontSize: 15, lineHeight: 1.6, color: 'var(--ink-soft)', maxWidth: 460 }}>
                Confirm the code on your server. Adagio receives a token; your password stays at home.
              </p>
              <div style={{ marginTop: 40, display: 'flex', gap: 24, alignItems: 'center', maxWidth: 540 }}>
                {/* Code card */}
                <div style={{ flex: 1, padding: 28, background: 'var(--ink)', color: 'var(--cream)', borderRadius: 'var(--r-3)', textAlign: 'center', minHeight: 120, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center' }}>
                  <div style={{ fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.2em', textTransform: 'uppercase', opacity: 0.55, marginBottom: 12 }}>Your one-time code</div>
                  {authFlow ? (
                    <div style={{ fontFamily: 'var(--mono)', fontSize: 32, letterSpacing: '0.18em', fontWeight: 500 }}>
                      {authFlow.display_code}
                    </div>
                  ) : authError ? (
                    <div style={{ fontSize: 13, color: 'var(--clay-soft)', maxWidth: 200 }}>{authError}</div>
                  ) : (
                    <SpinDot color="var(--cream)" />
                  )}
                  {authFlow && (
                    <div style={{ marginTop: 12, fontSize: 12, opacity: countdown < 60 ? 1 : 0.55, color: countdown < 60 ? 'var(--clay-soft)' : undefined }}>
                      expires in {String(Math.floor(countdown / 60)).padStart(2, '0')}:{String(countdown % 60).padStart(2, '0')}
                    </div>
                  )}
                </div>

                {/* QR code */}
                <div style={{ width: 100, height: 100, padding: 8, background: 'var(--cream)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                  {authFlow?.qr_svg ? (
                    <div
                      data-testid="qr-container"
                      dangerouslySetInnerHTML={{ __html: authFlow.qr_svg }}
                      style={{ width: '100%', height: '100%' }}
                    />
                  ) : (
                    <SpinDot color="var(--clay)" />
                  )}
                </div>
              </div>

              {/* Status row */}
              <div style={{ marginTop: 24, display: 'flex', alignItems: 'center', gap: 10, fontSize: 13, color: 'var(--ink-muted)' }}>
                {authExpired ? (
                  <span style={{ color: 'var(--clay)' }}>Session expired. Use "Try again" to restart.</span>
                ) : authError ? (
                  <>
                    <span style={{ color: 'var(--clay)' }}>Could not reach server. </span>
                    <button
                      onClick={() => goStep(2)}
                      style={{ background: 'transparent', border: 'none', color: 'var(--clay)', textDecoration: 'underline', cursor: 'pointer', fontSize: 13, padding: 0 }}
                    >
                      Go back
                    </button>
                  </>
                ) : authFlow ? (
                  <>
                    <SpinDot color="var(--clay)" />
                    <span>Waiting for confirmation at {hostname(url)}…</span>
                  </>
                ) : null}
              </div>
            </div>

            {/* Custom step-3 footer to support "Try again" */}
            <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 40, paddingTop: 20, borderTop: '1px solid var(--hairline)' }}>
              <button
                onClick={() => goStep(2)}
                style={{ background: 'transparent', border: 'none', color: 'var(--ink-muted)', cursor: 'pointer', fontSize: 13, display: 'flex', alignItems: 'center', gap: 6 }}
              >
                ← Back
              </button>
              <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
                <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)', letterSpacing: '0.06em' }}>step 3 of 5</span>
                {authExpired ? (
                  <button
                    onClick={handleTryAgain}
                    style={{ background: 'var(--ink)', color: 'var(--cream)', border: 'none', padding: '10px 22px', borderRadius: 'var(--r-pill)', fontSize: 13, fontWeight: 500, cursor: 'pointer' }}
                  >
                    Try again
                  </button>
                ) : (
                  <button
                    onClick={() => goStep(4)}
                    style={{ background: 'var(--ink)', color: 'var(--cream)', border: 'none', padding: '10px 22px', borderRadius: 'var(--r-pill)', fontSize: 13, fontWeight: 500, cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 8 }}
                  >
                    Continue <Icon name="arrow-r" size={14} color="var(--cream)" />
                  </button>
                )}
              </div>
            </div>
          </>
        )}

        {/* ── Step 4: Where to sync ───────────────────────────────────────── */}
        {step === 4 && (
          <>
            <div>
              <h1 style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 52, lineHeight: 1.02, margin: 0, letterSpacing: '-0.05em' }}>
                Pick a <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontWeight: 400, letterSpacing: '-0.01em' }}>quiet</span> folder.
              </h1>
              <p style={{ marginTop: 20, fontSize: 15, lineHeight: 1.6, color: 'var(--ink-soft)', maxWidth: 480 }}>
                This is where your cloud will live on disk. You can move it later.
              </p>
              <div style={{ marginTop: 32, maxWidth: 540 }}>
                <label style={MONO_LABEL}>Local folder</label>
                <div style={{ marginTop: 8, display: 'flex', alignItems: 'center', gap: 0, background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', padding: '12px 14px' }}>
                  <Icon name="folder" size={16} color="var(--ink-muted)" />
                  <input
                    value={folder}
                    onChange={e => setFolder(e.target.value)}
                    style={{ flex: 1, border: 'none', background: 'transparent', outline: 'none', fontFamily: 'var(--mono)', fontSize: 14, marginLeft: 10 }}
                  />
                  <button
                    onClick={handlePickFolder}
                    style={{ background: 'var(--cream-2)', border: 'none', padding: '5px 10px', borderRadius: 'var(--r-1)', fontSize: 12, cursor: 'pointer' }}
                  >
                    Browse…
                  </button>
                </div>
                <div style={{ marginTop: 16, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
                  <ToggleRow title="On-demand" desc="Files exist as placeholders until opened. Saves disk." value={vfsEnabled} onChange={setVfsEnabled} />
                  <ToggleRow title="Pin pinned folders" desc="Marked folders always live offline." value={pinPinned} onChange={setPinPinned} />
                  <ToggleRow title="Smart bandwidth" desc="Throttles when on battery or hotspot." value={smartBandwidth} onChange={setSmartBandwidth} />
                  <ToggleRow title="Watch external edits" desc="Detect changes from other apps instantly." value={watchExternal} onChange={setWatchExternal} />
                </div>
              </div>
            </div>
            <OnboardFooter step={step} setStep={goStep} />
          </>
        )}

        {/* ── Step 5: Begin ───────────────────────────────────────────────── */}
        {step === 5 && (
          <>
            <div style={{ marginTop: 40 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 14, marginBottom: 28 }}>
                <Icon name="check-circ" size={28} color="var(--forest)" />
                <div style={{ fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: 'var(--forest)' }}>
                  Connected to {hostname(url)}
                </div>
              </div>
              <h1 style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 64, lineHeight: 1, margin: 0, letterSpacing: '-0.05em' }}>
                Begin <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontWeight: 400, letterSpacing: '-0.01em' }}>softly.</span>
              </h1>
              <p style={{ marginTop: 22, fontSize: 16, lineHeight: 1.6, color: 'var(--ink-soft)', maxWidth: 500 }}>
                {remoteStatsFailed ? (
                  '— '
                ) : remoteStats ? (
                  remoteStats.file_count === 0 ? (
                    'No files yet · '
                  ) : remoteStats.file_count !== null ? (
                    `${remoteStats.file_count.toLocaleString()} files · ${remoteStats.total_bytes !== null ? formatBytes(remoteStats.total_bytes) + ' · ' : formatBytes(remoteStats.used_bytes) + ' used · '}`
                  ) : remoteStats.total_bytes !== null ? (
                    `${formatBytes(remoteStats.used_bytes)} used of ${formatBytes(remoteStats.total_bytes)} · `
                  ) : (
                    `${formatBytes(remoteStats.used_bytes)} used · `
                  )
                ) : (
                  'Fetching remote info… '
                )}
                We'll bring down the lightest files first; the rest stays in the cloud until you reach for it.
              </p>

              {pairError && (
                <div style={{ marginTop: 12, display: 'flex', alignItems: 'center', gap: 12 }}>
                  <p style={{ margin: 0, fontSize: 13, color: 'var(--clay)', fontFamily: 'var(--mono)' }}>
                    {pairError}
                  </p>
                  <button
                    onClick={() => { setPairError(null); setPairId(null); setStep(4); }}
                    style={{ background: 'transparent', border: '1px solid var(--clay)', color: 'var(--clay)', borderRadius: 'var(--r-1)', padding: '4px 10px', fontSize: 12, cursor: 'pointer', whiteSpace: 'nowrap' }}
                  >
                    Choose different folder
                  </button>
                </div>
              )}

              <div style={{ marginTop: 36, padding: 20, background: 'var(--paper-2)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', maxWidth: 540 }}>
                {pairCreating ? (
                  <div style={{ display: 'flex', alignItems: 'center', gap: 10, fontSize: 13, color: 'var(--ink-muted)' }}>
                    <SpinDot color="var(--clay)" />
                    Setting up sync folder…
                  </div>
                ) : (
                  <>
                    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
                      <span style={{ fontSize: 13.5, fontWeight: 500 }}>Initial sync</span>
                      <span style={{ fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--ink-muted)' }}>
                        {syncStatus
                      ? syncStatus.status === 'syncing' ? 'Syncing…'
                        : syncStatus.status === 'idle'   ? 'Up to date'
                        : syncStatus.status === 'paused' ? 'Resuming…'
                        : syncStatus.status
                      : 'Starting…'}
                      </span>
                    </div>
                    <div style={{ height: 6, background: 'var(--cream-2)', borderRadius: 3, overflow: 'hidden' }}>
                      <div style={{
                        width: syncStatus?.status === 'syncing' && syncStatus.total_bytes > 0
                          ? `${Math.round((syncStatus.transferred_bytes / syncStatus.total_bytes) * 100)}%`
                          : syncStatus?.status === 'idle' ? '100%' : '0%',
                        height: '100%',
                        background: syncStatus?.status === 'idle' ? 'var(--forest)' : 'var(--clay)',
                        transition: 'width 0.5s ease',
                      }}/>
                    </div>
                    {syncStatus?.status === 'syncing' && syncStatus.active_file_count > 0 && (
                      <div style={{ marginTop: 8, fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)' }}>
                        {syncStatus.active_file_count} file{syncStatus.active_file_count !== 1 ? 's' : ''} remaining
                      </div>
                    )}
                  </>
                )}
              </div>
            </div>
            <OnboardFooter step={step} setStep={goStep} finish continueDisabled={pairCreating || !!pairError} />
          </>
        )}

      </div>
    </div>
  );
}
