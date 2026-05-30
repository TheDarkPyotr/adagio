import React, { useState, useEffect } from 'react';
import { MarkSlur, Icon, SpinDot } from './shared';

function AdagioWord({ size = 32, color = 'var(--ink)', weight = 500 }: {
  size?: number; color?: string; weight?: number;
}) {
  return (
    <span style={{ fontFamily: 'var(--body)', fontWeight: weight, fontSize: size, lineHeight: 1, letterSpacing: '-0.05em', color }}>
      adagio
    </span>
  );
}

function FakeQR() {
  const cells = [
    '111110111','100010101','101010111','100010100','111110011','000001110','110101011','011010101','101011111',
  ];
  return (
    <div style={{ display: 'grid', gridTemplateColumns: 'repeat(9, 1fr)', gap: 1, width: '100%', height: '100%' }}>
      {cells.flatMap((r, ri) => r.split('').map((c, ci) =>
        <div key={`${ri}-${ci}`} style={{ background: c === '1' ? 'var(--ink)' : 'transparent' }}/>
      ))}
    </div>
  );
}

function ToggleRow({ title, desc, defaultOn = false }: { title: string; desc: string; defaultOn?: boolean }) {
  const [on, setOn] = useState(defaultOn);
  return (
    <button onClick={() => setOn(!on)}
      style={{ background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', padding: 14, textAlign: 'left', cursor: 'pointer', display: 'flex', flexDirection: 'column', gap: 4 }}>
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <span style={{ fontSize: 13.5, fontWeight: 500, color: 'var(--ink)' }}>{title}</span>
        <span style={{ width: 28, height: 16, background: on ? 'var(--forest)' : 'var(--cream-3)', borderRadius: 8, padding: 2, display: 'flex', justifyContent: on ? 'flex-end' : 'flex-start', transition: 'all .15s' }}>
          <span style={{ width: 12, height: 12, borderRadius: 6, background: 'var(--cream)', display: 'block' }}/>
        </span>
      </div>
      <span style={{ fontSize: 12, color: 'var(--ink-muted)', lineHeight: 1.45 }}>{desc}</span>
    </button>
  );
}

function OnboardFooter({ step, setStep, finish }: { step: number; setStep: (n: number) => void; finish?: boolean }) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 40, paddingTop: 20, borderTop: '1px solid var(--hairline)' }}>
      <button onClick={() => setStep(Math.max(1, step - 1))}
        style={{ background: 'transparent', border: 'none', color: 'var(--ink-muted)', cursor: 'pointer', fontSize: 13, display: 'flex', alignItems: 'center', gap: 6 }}>
        ← Back
      </button>
      <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
        <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)', letterSpacing: '0.06em' }}>step {step} of 5</span>
        <button onClick={() => setStep(finish ? step + 1 : Math.min(5, step + 1))}
          style={{ background: 'var(--ink)', color: 'var(--cream)', border: 'none', padding: '10px 22px', borderRadius: 'var(--r-pill)', fontSize: 13, fontWeight: 500, cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 8 }}>
          {finish ? 'Open Adagio' : 'Continue'} <Icon name="arrow-r" size={14} color="var(--cream)" />
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

export default function OnboardingWizard({ onComplete }: { onComplete: () => void }) {
  const [step, setStep] = useState(1);
  const [url, setUrl] = useState('https://cloud.sound.studio');
  const [folder, setFolder] = useState('~/Adagio');
  const [countdown, setCountdown] = useState(5 * 60); // 5 minutes in seconds

  useEffect(() => {
    if (step !== 3) return;
    setCountdown(5 * 60);
    const t = setInterval(() => setCountdown(s => (s > 0 ? s - 1 : 0)), 1000);
    return () => clearInterval(t);
  }, [step]);

  const goStep = (n: number) => {
    if (n > 5) { onComplete(); return; }
    setStep(n);
  };

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
                <div style={{ marginTop: 8, display: 'flex', alignItems: 'center', gap: 0, background: 'var(--paper)', border: '1.5px solid var(--ink)', borderRadius: 'var(--r-2)', padding: '12px 14px' }}>
                  <Icon name="globe" size={16} color="var(--ink-muted)" />
                  <input value={url} onChange={e => setUrl(e.target.value)}
                    style={{ flex: 1, border: 'none', background: 'transparent', outline: 'none', fontFamily: 'var(--mono)', fontSize: 14, color: 'var(--ink)', marginLeft: 10 }}/>
                  <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--good)', display: 'flex', alignItems: 'center', gap: 6 }}>
                    <span style={{ width: 6, height: 6, borderRadius: 3, background: 'var(--good)' }}/>Nextcloud 28.0.1
                  </span>
                </div>
                <div style={{ marginTop: 12, padding: 14, background: 'var(--paper-2)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)' }}>
                  <div style={{ fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.16em', textTransform: 'uppercase', color: 'var(--ink-muted)', marginBottom: 10 }}>Detected</div>
                  <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 10, fontSize: 13 }}>
                    <div><span style={{ color: 'var(--ink-muted)' }}>Auth flow</span><br/><span style={{ fontFamily: 'var(--mono)' }}>OAuth2 · device flow</span></div>
                    <div><span style={{ color: 'var(--ink-muted)' }}>TLS</span><br/><span style={{ fontFamily: 'var(--mono)', color: 'var(--good)' }}>Let's Encrypt, valid</span></div>
                    <div><span style={{ color: 'var(--ink-muted)' }}>E2EE</span><br/><span style={{ fontFamily: 'var(--mono)' }}>available · per-folder</span></div>
                    <div><span style={{ color: 'var(--ink-muted)' }}>Reach</span><br/><span style={{ fontFamily: 'var(--mono)' }}>146 ms · Frankfurt</span></div>
                  </div>
                </div>
              </div>
            </div>
            <OnboardFooter step={step} setStep={goStep} />
          </>
        )}

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
                <div style={{ flex: 1, padding: 28, background: 'var(--ink)', color: 'var(--cream)', borderRadius: 'var(--r-3)', textAlign: 'center' }}>
                  <div style={{ fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.2em', textTransform: 'uppercase', opacity: 0.55, marginBottom: 12 }}>Your one-time code</div>
                  <div style={{ fontFamily: 'var(--mono)', fontSize: 32, letterSpacing: '0.18em', fontWeight: 500 }}>F4PS · 9TRX</div>
                  <div style={{ marginTop: 12, fontSize: 12, opacity: countdown < 60 ? 1 : 0.55, color: countdown < 60 ? 'var(--clay-soft)' : undefined }}>
                    expires in {String(Math.floor(countdown / 60)).padStart(2, '0')}:{String(countdown % 60).padStart(2, '0')}
                  </div>
                </div>
                <div style={{ width: 100, height: 100, padding: 8, background: 'var(--cream)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)' }}>
                  <FakeQR />
                </div>
              </div>
              <div style={{ marginTop: 24, display: 'flex', alignItems: 'center', gap: 10, fontSize: 13, color: 'var(--ink-muted)' }}>
                <SpinDot color="var(--clay)" />
                <span>Waiting for confirmation at {new URL(url).hostname}…</span>
              </div>
            </div>
            <OnboardFooter step={step} setStep={goStep} />
          </>
        )}

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
                  <input value={folder} onChange={e => setFolder(e.target.value)}
                    style={{ flex: 1, border: 'none', background: 'transparent', outline: 'none', fontFamily: 'var(--mono)', fontSize: 14, marginLeft: 10 }}/>
                  <button style={{ background: 'var(--cream-2)', border: 'none', padding: '5px 10px', borderRadius: 'var(--r-1)', fontSize: 12, cursor: 'pointer' }}>Browse…</button>
                </div>
                <div style={{ marginTop: 16, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
                  <ToggleRow title="On-demand" desc="Files exist as placeholders until opened. Saves disk." defaultOn />
                  <ToggleRow title="Pin pinned folders" desc="Marked folders always live offline." defaultOn />
                  <ToggleRow title="Smart bandwidth" desc="Throttles when on battery or hotspot." defaultOn />
                  <ToggleRow title="Watch external edits" desc="Detect changes from other apps instantly." />
                </div>
              </div>
            </div>
            <OnboardFooter step={step} setStep={goStep} />
          </>
        )}

        {step === 5 && (
          <>
            <div style={{ marginTop: 40 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 14, marginBottom: 28 }}>
                <Icon name="check-circ" size={28} color="var(--forest)" />
                <div style={{ fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: 'var(--forest)' }}>
                  Connected to {(() => { try { return new URL(url).hostname; } catch { return url; } })()}
                </div>
              </div>
              <h1 style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 64, lineHeight: 1, margin: 0, letterSpacing: '-0.05em' }}>
                Begin <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontWeight: 400, letterSpacing: '-0.01em' }}>softly.</span>
              </h1>
              <p style={{ marginTop: 22, fontSize: 16, lineHeight: 1.6, color: 'var(--ink-soft)', maxWidth: 500 }}>
                3,247 files · 86.7 GB. We'll bring down the lightest 200 first; the rest stays in the cloud until you reach for it.
              </p>
              <div style={{ marginTop: 36, padding: 20, background: 'var(--paper-2)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', maxWidth: 540 }}>
                <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
                  <span style={{ fontSize: 13.5, fontWeight: 500 }}>Initial sync</span>
                  <span style={{ fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--ink-muted)' }}>200 of 3,247 · 12 MB / 18 MB</span>
                </div>
                <div style={{ height: 6, background: 'var(--cream-2)', borderRadius: 3, overflow: 'hidden' }}>
                  <div style={{ width: '64%', height: '100%', background: 'var(--clay)' }}/>
                </div>
              </div>
            </div>
            <OnboardFooter step={step} setStep={goStep} finish />
          </>
        )}
      </div>
    </div>
  );
}

const MONO_LABEL: React.CSSProperties = {
  fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.16em',
  textTransform: 'uppercase', color: 'var(--ink-muted)',
};
