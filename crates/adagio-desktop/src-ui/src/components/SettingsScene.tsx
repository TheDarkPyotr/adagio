import React, { useState, useEffect, useRef } from 'react';
import { Icon, SpinDot } from './shared';
import type { SyncStatusDto, DaemonStatusDto, BandwidthStatusDto, NetworkStatusDto, NetworkAction } from '../tauri';
import { setPalette as ipcSetPalette, pauseSync, resumeSync, getDaemonStatus, startDaemon, stopDaemon, setStartAtLogin, getBandwidthStatus, setBandwidthLimits, clearBandwidthLimits, getNetworkStatus, setNetworkPolicy, addBlockedSsid, removeBlockedSsid } from '../tauri';

type Section = 'appearance' | 'sync' | 'about';

const PALETTES = [
  { id: 'sienna', label: 'Sienna', cream: '#f5f1ea', ink: '#15171a', accent: '#c8542a' },
  { id: 'slate',  label: 'Slate',  cream: '#e9ecef', ink: '#0e1116', accent: '#b76b35' },
  { id: 'bone',   label: 'Bone',   cream: '#ece4d4', ink: '#1c1815', accent: '#a4452f' },
  { id: 'ink',    label: 'Ink',    cream: '#15181c', ink: '#f0ece4', accent: '#e07a4d' },
  { id: 'plum',   label: 'Plum',   cream: '#efe5e3', ink: '#2a1422', accent: '#c66648' },
  { id: 'azure',  label: 'Azure',  cream: '#e8edf3', ink: '#0d1a2e', accent: '#2f6fcf' },
  { id: 'iris',   label: 'Iris',   cream: '#ebe3ef', ink: '#1f1729', accent: '#9550b8' },
  { id: 'citron', label: 'Citron', cream: '#f1ecd2', ink: '#1c1810', accent: '#c89020' },
];

export default function SettingsScene({ palette, onPalette, syncStatus, onBack, onPairs }: {
  palette: string;
  onPalette: (name: string) => void;
  syncStatus: SyncStatusDto | null;
  onBack: () => void;
  onPairs: () => void;
}) {
  const [section, setSection] = useState<Section>('appearance');
  const [pausing, setPausing] = useState(false);
  const [daemonStatus, setDaemonStatus] = useState<DaemonStatusDto | null>(null);
  const [daemonLoading, setDaemonLoading] = useState(false);
  const [bandwidthStatus, setBandwidthStatus] = useState<BandwidthStatusDto | null>(null);
  const [uploadInput, setUploadInput] = useState('');
  const [downloadInput, setDownloadInput] = useState('');
  const [bandwidthSaving, setBandwidthSaving] = useState(false);
  const bandwidthPollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Network awareness state
  const [networkStatus, setNetworkStatus] = useState<NetworkStatusDto | null>(null);
  const [onMetered, setOnMetered] = useState<NetworkAction>('allow');
  const [onBattery, setOnBattery] = useState<NetworkAction>('allow');
  const [networkThrottle, setNetworkThrottle] = useState('');
  const [newSsid, setNewSsid] = useState('');
  const [networkSaving, setNetworkSaving] = useState(false);
  const networkPollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  // Load daemon + bandwidth + network status when the Sync section is shown.
  useEffect(() => {
    if (section !== 'sync') {
      if (bandwidthPollRef.current) { clearInterval(bandwidthPollRef.current); bandwidthPollRef.current = null; }
      return;
    }
    getDaemonStatus().then(setDaemonStatus).catch(() => {});

    const refreshBandwidth = () => getBandwidthStatus().then(s => {
      setBandwidthStatus(s);
      setUploadInput(prev => prev === '' ? (s.upload_limit_kbps === 0 ? '' : String(s.upload_limit_kbps)) : prev);
      setDownloadInput(prev => prev === '' ? (s.download_limit_kbps === 0 ? '' : String(s.download_limit_kbps)) : prev);
    }).catch(() => {});
    refreshBandwidth();
    bandwidthPollRef.current = setInterval(refreshBandwidth, 3000);

    const refreshNetwork = () => getNetworkStatus().then(s => {
      setNetworkStatus(s);
      setOnMetered(s.policy.on_metered);
      setOnBattery(s.policy.on_battery);
      setNetworkThrottle(s.policy.throttle_kbps === 0 ? '' : String(s.policy.throttle_kbps));
    }).catch(() => {});
    refreshNetwork();
    networkPollRef.current = setInterval(refreshNetwork, 3000);

    return () => {
      if (bandwidthPollRef.current) { clearInterval(bandwidthPollRef.current); bandwidthPollRef.current = null; }
      if (networkPollRef.current) { clearInterval(networkPollRef.current); networkPollRef.current = null; }
    };
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [section]);

  const handlePauseResume = async () => {
    setPausing(true);
    try {
      if (syncStatus?.status === 'paused') await resumeSync();
      else await pauseSync();
    } catch {}
    setPausing(false);
  };

  const NAV: [Section | 'pairs', string][] = [
    ['appearance', 'Appearance'],
    ['sync',       'Sync'],
    ['pairs',      'Sync pairs'],
    ['about',      'About'],
  ];

  return (
    <div style={{ flex: 1, display: 'flex', overflow: 'hidden', background: 'var(--cream)' }}>
      {/* left nav */}
      <div style={{ width: 220, flexShrink: 0, borderRight: '1px solid var(--hairline)', background: 'var(--paper)', display: 'flex', flexDirection: 'column', padding: '20px 10px' }}>
        <button onClick={onBack}
          style={{ display: 'flex', alignItems: 'center', gap: 8, background: 'transparent', border: 'none', cursor: 'pointer', color: 'var(--ink-muted)', fontSize: 13, padding: '6px 10px', borderRadius: 'var(--r-2)', marginBottom: 16, textAlign: 'left' }}>
          <div style={{ transform: 'rotate(180deg)', display: 'flex' }}>
            <Icon name="arrow-r" size={13} color="var(--ink-muted)" />
          </div>
          Back to files
        </button>

        <div style={{ padding: '2px 10px 8px', fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.16em', textTransform: 'uppercase', color: 'var(--ink-muted)' }}>Settings</div>

        {NAV.map(([id, label]) => {
          const active = id !== 'pairs' && section === id;
          return (
            <button key={id}
              onClick={() => id === 'pairs' ? onPairs() : setSection(id as Section)}
              style={{ background: active ? 'var(--cream-2)' : 'transparent', color: active ? 'var(--ink)' : 'var(--ink-soft)', border: 'none', borderRadius: 'var(--r-2)', padding: '8px 10px', fontSize: 13, cursor: 'pointer', fontWeight: active ? 500 : 400, textAlign: 'left', display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 1 }}>
              {label}
              {id === 'pairs' && <Icon name="chevron" size={13} color="var(--ink-muted)" />}
            </button>
          );
        })}
      </div>

      {/* content */}
      <div style={{ flex: 1, overflowY: 'auto', padding: '36px 52px' }}>

        {section === 'appearance' && (
          <div style={{ maxWidth: 560 }}>
            <h2 style={SECTION_H2}>Appearance</h2>
            <p style={SECTION_P}>Choose a colour palette. The change applies instantly and is saved across restarts.</p>

            <div style={FIELD_LABEL}>Palette</div>
            <div style={{ marginTop: 10, display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 10 }}>
              {PALETTES.map(p => {
                const active = palette === p.id;
                return (
                  <button key={p.id} onClick={() => onPalette(p.id)}
                    style={{ background: p.cream, border: active ? `2px solid ${p.accent}` : '1.5px solid rgba(0,0,0,0.09)', borderRadius: 'var(--r-2)', padding: '12px 10px 10px', cursor: 'pointer', position: 'relative', textAlign: 'left', transition: 'border-color 0.12s' }}>
                    <div style={{ display: 'flex', gap: 5, marginBottom: 8 }}>
                      <div style={{ width: 16, height: 16, borderRadius: 8, background: p.accent }} />
                      <div style={{ width: 16, height: 16, borderRadius: 8, background: p.ink }} />
                    </div>
                    <div style={{ fontFamily: 'var(--body)', fontSize: 12, fontWeight: 500, color: p.ink }}>{p.label}</div>
                    {active && (
                      <div style={{ position: 'absolute', top: 6, right: 6, width: 16, height: 16, borderRadius: 8, background: p.accent, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                        <Icon name="check" size={9} color={p.cream} strokeWidth={3} />
                      </div>
                    )}
                  </button>
                );
              })}
            </div>
          </div>
        )}

        {section === 'sync' && (
          <div style={{ maxWidth: 520 }}>
            <h2 style={SECTION_H2}>Sync</h2>
            <p style={SECTION_P}>Control how Adagio synchronises files in the background.</p>

            <div style={{ display: 'flex', flexDirection: 'column' }}>
              <SettingsRow
                title="Sync status"
                desc={
                  syncStatus?.status === 'paused' ? 'Paused — no files will be transferred.' :
                  syncStatus?.status === 'syncing' ? `Syncing ${syncStatus.active_file_count} file${syncStatus.active_file_count !== 1 ? 's' : ''}` :
                  syncStatus?.status === 'error'   ? 'Error — check the activity log.' :
                  'Up to date'
                }
              >
                <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
                  {syncStatus?.status === 'syncing' && <SpinDot color="var(--clay)" />}
                  <button onClick={handlePauseResume} disabled={pausing}
                    style={{ background: syncStatus?.status === 'paused' ? 'var(--forest)' : 'var(--paper-2)', color: syncStatus?.status === 'paused' ? 'var(--cream)' : 'var(--ink)', border: '1px solid var(--hairline)', padding: '7px 16px', borderRadius: 'var(--r-pill)', fontSize: 12.5, cursor: 'pointer', fontWeight: 500, opacity: pausing ? 0.5 : 1, transition: 'opacity 0.12s' }}>
                    {syncStatus?.status === 'paused' ? 'Resume' : 'Pause'}
                  </button>
                </div>
              </SettingsRow>

              <Divider />

              <SettingsRow
                title="Last synced"
                desc={
                  syncStatus?.last_sync_at
                    ? new Intl.DateTimeFormat(undefined, { hour: '2-digit', minute: '2-digit', month: 'short', day: 'numeric' }).format(new Date(syncStatus.last_sync_at))
                    : 'Never'
                }
              />

              <Divider />

              <SettingsRow title="Sync pairs" desc="Manage which local folders are paired with your Nextcloud.">
                <button onClick={onPairs}
                  style={{ background: 'transparent', border: '1px solid var(--hairline)', padding: '7px 14px', borderRadius: 'var(--r-pill)', fontSize: 12.5, cursor: 'pointer', color: 'var(--ink)', fontWeight: 500, display: 'flex', alignItems: 'center', gap: 6 }}>
                  Manage <Icon name="chevron" size={12} color="var(--ink-soft)" />
                </button>
              </SettingsRow>

              <Divider />

              {/* Background sync daemon controls */}
              <SettingsRow
                title="Background sync"
                desc={
                  daemonStatus == null ? 'Loading…' :
                  daemonStatus.running ? `Running${daemonStatus.uptime_secs != null ? ` · up ${Math.floor(daemonStatus.uptime_secs / 60)} min` : ''}` :
                  'Stopped — files will not sync while Adagio is closed.'
                }
              >
                <button
                  disabled={daemonLoading}
                  onClick={async () => {
                    setDaemonLoading(true);
                    try {
                      if (daemonStatus?.running) {
                        await stopDaemon();
                        setDaemonStatus(prev => prev ? { ...prev, running: false, connection_state: 'stopped' } : null);
                      } else {
                        await startDaemon();
                        const updated = await getDaemonStatus();
                        setDaemonStatus(updated);
                      }
                    } catch {}
                    setDaemonLoading(false);
                  }}
                  style={{
                    background: daemonStatus?.running ? 'var(--paper-2)' : 'var(--forest)',
                    color: daemonStatus?.running ? 'var(--ink)' : 'var(--cream)',
                    border: '1px solid var(--hairline)', padding: '7px 16px',
                    borderRadius: 'var(--r-pill)', fontSize: 12.5, cursor: 'pointer',
                    fontWeight: 500, opacity: daemonLoading ? 0.5 : 1,
                  }}
                >
                  {daemonLoading ? '…' : daemonStatus?.running ? 'Stop background sync' : 'Start background sync'}
                </button>
              </SettingsRow>

              <Divider />

              <SettingsRow
                title="Start at login"
                desc="Automatically start background sync when you log in to your computer."
              >
                <label style={{ display: 'flex', alignItems: 'center', gap: 8, cursor: 'pointer', fontSize: 13 }}>
                  <input
                    type="checkbox"
                    defaultChecked={true}
                    onChange={e => setStartAtLogin(e.target.checked).catch(() => {})}
                  />
                  Enable
                </label>
              </SettingsRow>

              <Divider />

              {/* Bandwidth throttling */}
              <div style={{ padding: '14px 0' }}>
                <div style={{ fontSize: 14, fontWeight: 500, color: 'var(--ink)', marginBottom: 4 }}>Bandwidth</div>
                <div style={{ fontSize: 12.5, color: 'var(--ink-muted)', lineHeight: 1.5, marginBottom: 14 }}>
                  Limit how much network bandwidth Adagio uses. Leave blank for unlimited.
                </div>
                <div style={{ display: 'flex', gap: 16, flexWrap: 'wrap', marginBottom: 14 }}>
                  <div>
                    <div style={FIELD_LABEL}>Upload limit (Kbps)</div>
                    <input
                      data-testid="upload-limit-input"
                      type="number"
                      min={0}
                      placeholder="Unlimited"
                      value={uploadInput}
                      onChange={e => setUploadInput(e.target.value)}
                      style={{ marginTop: 6, padding: '6px 10px', fontSize: 13, border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', background: 'var(--paper)', color: 'var(--ink)', width: 140 }}
                    />
                  </div>
                  <div>
                    <div style={FIELD_LABEL}>Download limit (Kbps)</div>
                    <input
                      data-testid="download-limit-input"
                      type="number"
                      min={0}
                      placeholder="Unlimited"
                      value={downloadInput}
                      onChange={e => setDownloadInput(e.target.value)}
                      style={{ marginTop: 6, padding: '6px 10px', fontSize: 13, border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', background: 'var(--paper)', color: 'var(--ink)', width: 140 }}
                    />
                  </div>
                </div>
                <div style={{ display: 'flex', gap: 10, alignItems: 'center' }}>
                  <button
                    data-testid="bandwidth-save-btn"
                    disabled={bandwidthSaving}
                    onClick={async () => {
                      setBandwidthSaving(true);
                      try {
                        const up = parseInt(uploadInput, 10) || 0;
                        const dl = parseInt(downloadInput, 10) || 0;
                        await setBandwidthLimits(up, dl);
                        const updated = await getBandwidthStatus();
                        setBandwidthStatus(updated);
                      } catch {}
                      setBandwidthSaving(false);
                    }}
                    style={{ background: 'var(--clay)', color: 'var(--cream)', border: 'none', padding: '7px 16px', borderRadius: 'var(--r-pill)', fontSize: 12.5, cursor: 'pointer', fontWeight: 500, opacity: bandwidthSaving ? 0.5 : 1 }}
                  >
                    {bandwidthSaving ? 'Saving…' : 'Save'}
                  </button>
                  <button
                    data-testid="bandwidth-clear-btn"
                    disabled={bandwidthSaving}
                    onClick={async () => {
                      setBandwidthSaving(true);
                      try {
                        await clearBandwidthLimits();
                        setUploadInput('');
                        setDownloadInput('');
                        const updated = await getBandwidthStatus();
                        setBandwidthStatus(updated);
                      } catch {}
                      setBandwidthSaving(false);
                    }}
                    style={{ background: 'transparent', border: '1px solid var(--hairline)', color: 'var(--ink)', padding: '7px 14px', borderRadius: 'var(--r-pill)', fontSize: 12.5, cursor: 'pointer', fontWeight: 500 }}
                  >
                    Clear
                  </button>
                </div>
                {bandwidthStatus && (
                  <div style={{ marginTop: 14, fontSize: 12, color: 'var(--ink-muted)', fontFamily: 'var(--mono)', display: 'flex', gap: 24 }}>
                    <span data-testid="live-upload-rate">Upload: {bandwidthStatus.upload_rate_kbps} Kbps</span>
                    <span data-testid="live-download-rate">Download: {bandwidthStatus.download_rate_kbps} Kbps</span>
                  </div>
                )}
              </div>

              <Divider />

              {/* Network awareness */}
              <div style={{ padding: '14px 0' }}>
                <div style={{ fontSize: 14, fontWeight: 500, color: 'var(--ink)', marginBottom: 4 }}>Network</div>
                <div style={{ fontSize: 12.5, color: 'var(--ink-muted)', lineHeight: 1.5, marginBottom: 14 }}>
                  Automatically pause or throttle sync based on connection type or power state.
                </div>

                <div style={{ display: 'flex', gap: 16, flexWrap: 'wrap', marginBottom: 14 }}>
                  <div>
                    <div style={FIELD_LABEL}>On metered connection</div>
                    <select
                      data-testid="network-on-metered-select"
                      value={onMetered}
                      onChange={e => setOnMetered(e.target.value as NetworkAction)}
                      style={{ marginTop: 6, padding: '6px 10px', fontSize: 13, border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', background: 'var(--paper)', color: 'var(--ink)' }}
                    >
                      <option value="allow">Allow</option>
                      <option value="throttle">Throttle</option>
                      <option value="pause">Pause</option>
                    </select>
                  </div>
                  <div>
                    <div style={FIELD_LABEL}>On battery</div>
                    <select
                      data-testid="network-on-battery-select"
                      value={onBattery}
                      onChange={e => setOnBattery(e.target.value as NetworkAction)}
                      style={{ marginTop: 6, padding: '6px 10px', fontSize: 13, border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', background: 'var(--paper)', color: 'var(--ink)' }}
                    >
                      <option value="allow">Allow</option>
                      <option value="throttle">Throttle</option>
                      <option value="pause">Pause</option>
                    </select>
                  </div>
                  {(onMetered === 'throttle' || onBattery === 'throttle') && (
                    <div>
                      <div style={FIELD_LABEL}>Throttle limit (Kbps)</div>
                      <input
                        data-testid="network-throttle-input"
                        type="number"
                        min={1}
                        placeholder="e.g. 200"
                        value={networkThrottle}
                        onChange={e => setNetworkThrottle(e.target.value)}
                        style={{ marginTop: 6, padding: '6px 10px', fontSize: 13, border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', background: 'var(--paper)', color: 'var(--ink)', width: 120 }}
                      />
                    </div>
                  )}
                </div>

                <button
                  data-testid="network-save-btn"
                  disabled={networkSaving}
                  onClick={async () => {
                    setNetworkSaving(true);
                    try {
                      const kbps = parseInt(networkThrottle, 10) || 0;
                      await setNetworkPolicy(onMetered, onBattery, kbps);
                      const updated = await getNetworkStatus();
                      setNetworkStatus(updated);
                    } catch {}
                    setNetworkSaving(false);
                  }}
                  style={{ background: 'var(--clay)', color: 'var(--cream)', border: 'none', padding: '7px 16px', borderRadius: 'var(--r-pill)', fontSize: 12.5, cursor: 'pointer', fontWeight: 500, opacity: networkSaving ? 0.5 : 1, marginBottom: 16 }}
                >
                  {networkSaving ? 'Saving…' : 'Save'}
                </button>

                {/* SSID block list */}
                <div style={{ marginBottom: 8, fontSize: 12.5, fontWeight: 500, color: 'var(--ink)' }}>Blocked SSIDs</div>
                <div style={{ display: 'flex', gap: 8, marginBottom: 10 }}>
                  <input
                    data-testid="network-ssid-input"
                    type="text"
                    placeholder="Network name"
                    value={newSsid}
                    onChange={e => setNewSsid(e.target.value)}
                    style={{ padding: '6px 10px', fontSize: 13, border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', background: 'var(--paper)', color: 'var(--ink)', flex: 1, maxWidth: 220 }}
                  />
                  <button
                    data-testid="network-add-ssid-btn"
                    disabled={!newSsid.trim()}
                    onClick={async () => {
                      if (!newSsid.trim()) return;
                      try {
                        await addBlockedSsid(newSsid.trim());
                        setNewSsid('');
                        const updated = await getNetworkStatus();
                        setNetworkStatus(updated);
                      } catch {}
                    }}
                    style={{ background: 'var(--paper-2)', border: '1px solid var(--hairline)', color: 'var(--ink)', padding: '6px 14px', borderRadius: 'var(--r-2)', fontSize: 12.5, cursor: 'pointer' }}
                  >
                    Block
                  </button>
                </div>
                {networkStatus && networkStatus.policy.blocked_ssids.length > 0 && (
                  <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                    {networkStatus.policy.blocked_ssids.map(ssid => (
                      <div key={ssid} style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 13 }}>
                        <span style={{ fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--ink)' }}>{ssid}</span>
                        <button
                          onClick={async () => {
                            try {
                              await removeBlockedSsid(ssid);
                              const updated = await getNetworkStatus();
                              setNetworkStatus(updated);
                            } catch {}
                          }}
                          style={{ background: 'transparent', border: 'none', cursor: 'pointer', color: 'var(--ink-muted)', fontSize: 11 }}
                        >
                          ✕
                        </button>
                      </div>
                    ))}
                  </div>
                )}

                {/* Live network status */}
                {networkStatus && (
                  <div style={{ marginTop: 14, fontSize: 12, color: 'var(--ink-muted)', fontFamily: 'var(--mono)', display: 'flex', gap: 16, flexWrap: 'wrap' }}>
                    <span data-testid="network-metered-state">Metered: {networkStatus.metered ? 'Yes' : 'No'}</span>
                    <span data-testid="network-battery-state">Battery: {networkStatus.on_battery ? 'Yes' : 'No'}</span>
                    <span data-testid="network-ssid-state">SSID: {networkStatus.ssid ?? 'Unknown'}</span>
                    <span data-testid="network-effective-action">
                      {networkStatus.effective_action}{networkStatus.reason ? ` (${networkStatus.reason})` : ''}
                    </span>
                  </div>
                )}
              </div>
            </div>
          </div>
        )}

        {section === 'about' && (
          <div style={{ maxWidth: 460 }}>
            <h2 style={SECTION_H2}>About</h2>

            <div style={{ display: 'flex', alignItems: 'center', gap: 16, marginTop: 8, padding: 20, background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-3)' }}>
              <div style={{ width: 52, height: 52, borderRadius: 12, background: 'var(--ink)', flexShrink: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
                <span style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 38, color: 'var(--cream)', letterSpacing: '-0.08em', lineHeight: 1 }}>a</span>
              </div>
              <div>
                <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 16, letterSpacing: '-0.04em' }}>adagio</div>
                <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)', marginTop: 3 }}>v1.0 · allegretto · for Nextcloud</div>
              </div>
            </div>

            <div style={{ marginTop: 24, display: 'flex', flexDirection: 'column' }}>
              <SettingsRow title="License" desc="GNU Affero General Public License v3.0" />
              <Divider />
              <SettingsRow title="Source code" desc="Available on GitHub under AGPL-3.0" />
              <Divider />
              <SettingsRow title="Telemetry" desc="None. Adagio does not phone home, ever." />
              <Divider />
              <SettingsRow title="Crash logs" desc="Stored locally — only sent if you choose to report." />
            </div>
          </div>
        )}
      </div>
    </div>
  );
}

function SettingsRow({ title, desc, children }: { title: string; desc: string; children?: React.ReactNode }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '14px 0', gap: 24, minHeight: 56 }}>
      <div>
        <div style={{ fontSize: 14, fontWeight: 500, color: 'var(--ink)' }}>{title}</div>
        <div style={{ fontSize: 12.5, color: 'var(--ink-muted)', marginTop: 2, lineHeight: 1.5 }}>{desc}</div>
      </div>
      {children && <div style={{ flexShrink: 0 }}>{children}</div>}
    </div>
  );
}

function Divider() {
  return <div style={{ height: 1, background: 'var(--hairline)' }} />;
}

const SECTION_H2: React.CSSProperties = { fontFamily: 'var(--body)', fontWeight: 500, fontSize: 22, letterSpacing: '-0.04em', margin: '0 0 8px' };
const SECTION_P: React.CSSProperties = { fontSize: 13.5, color: 'var(--ink-soft)', lineHeight: 1.6, margin: '0 0 28px' };
const FIELD_LABEL: React.CSSProperties = { fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.16em', textTransform: 'uppercase', color: 'var(--ink-muted)' };
