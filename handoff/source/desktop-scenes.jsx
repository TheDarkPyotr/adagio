/* desktop-scenes.jsx — Files / Onboard / Activity / Share / Tray + DesktopApp dispatcher */

const { useState: useS } = React;

/* ─── Files scene ─── */

function FilesScene({ files, onShare, sourceFilter, miniature }) {
  const [selected, setSel] = useS(2); // pre-selected the .md file so the share dialog has context
  const visible = files;

  return (
    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden', background: 'var(--cream)' }}>
      {/* breadcrumb / actions row */}
      <div style={{ height: 52, flexShrink: 0, padding: '0 22px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', borderBottom: '1px solid var(--hairline)' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, fontSize: 13 }}>
          <span style={{ color: 'var(--ink-muted)' }}>cloud.sound.studio</span>
          <Icon name="chevron" size={11} color="var(--ink-muted)" />
          <span style={{ color: 'var(--ink-muted)' }}>Sound</span>
          <Icon name="chevron" size={11} color="var(--ink-muted)" />
          <span style={{ color: 'var(--ink)', fontWeight: 600, fontFamily: 'var(--body)', fontSize: 14, letterSpacing: '-0.02em', marginLeft: 4 }}>{sourceFilter === 'fav' ? 'Favorites' : sourceFilter === 'shared' ? 'Shared with me' : 'Sound'}</span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <FlatBtn icon="plus" label="New" />
          <FlatBtn icon="cloud-dl" label="Make available offline" />
          <FlatBtn icon="share" label="Share" onClick={() => onShare && onShare()} primary />
        </div>
      </div>

      {/* table */}
      <div style={{ flex: 1, overflow: 'hidden', display: 'flex', flexDirection: 'column' }}>
        <div style={{ display: 'grid', gridTemplateColumns: '38px 1fr 110px 110px 160px 80px', padding: '12px 22px 12px',
          fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.14em', textTransform: 'uppercase', color: 'var(--ink-muted)',
          borderBottom: '1px solid var(--hairline)' }}>
          <span/>
          <span>Name</span>
          <span>Size</span>
          <span>Items</span>
          <span>Modified</span>
          <span style={{ textAlign: 'right' }}>Status</span>
        </div>

        <div style={{ flex: 1, overflowY: 'auto', padding: '4px 0 12px' }}>
          {visible.map((f, i) => (
            <FileRow key={i} file={f} selected={selected === i} onClick={() => setSel(i)} onShare={onShare} />
          ))}
        </div>
      </div>

      {/* status bar */}
      <div style={{ height: 32, flexShrink: 0, borderTop: '1px solid var(--hairline)', background: 'var(--paper)',
        display: 'flex', alignItems: 'center', justifyContent: 'space-between', padding: '0 20px',
        fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)', letterSpacing: '0.04em' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 18 }}>
          <span>{visible.length} items · {selected !== null ? '1 selected' : 'nothing selected'}</span>
          <span>·</span>
          <span>2.1 GB local · 84.6 GB on cloud</span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          <span style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <span style={{ width: 6, height: 6, borderRadius: 3, background: 'var(--clay)' }}/>
            syncing · ETA 38s
          </span>
          <span>·</span>
          <span>last full sync 11:42</span>
        </div>
      </div>
    </div>
  );
}

function FileRow({ file, selected, onClick, onShare }) {
  const [h, setH] = useS(false);
  return (
    <div onClick={onClick}
      onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{
        display: 'grid', gridTemplateColumns: '38px 1fr 110px 110px 160px 80px',
        padding: '11px 22px', alignItems: 'center',
        background: selected ? 'var(--paper-2)' : h ? 'color-mix(in srgb, var(--ink) 4%, transparent)' : 'transparent',
        cursor: 'pointer',
        boxShadow: selected ? 'inset 2px 0 0 var(--clay)' : 'none',
        fontSize: 13.5,
      }}>
      <FileGlyph kind={file.kind} status={file.status} />
      <div style={{ display: 'flex', alignItems: 'center', gap: 10, minWidth: 0 }}>
        <span style={{ color: 'var(--ink)', fontWeight: file.kind === 'folder' ? 500 : 400, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{file.name}</span>
        {file.share && (
          <span title={`Shared with ${file.share}`} style={{ display: 'flex', alignItems: 'center', gap: 3, color: 'var(--ink-muted)', fontSize: 11, fontFamily: 'var(--mono)' }}>
            <Icon name="people" size={12} color="var(--ink-muted)" />
            <span>{file.share}</span>
          </span>
        )}
      </div>
      <span style={{ color: 'var(--ink-muted)', fontSize: 12, fontFamily: 'var(--mono)' }}>{file.size}</span>
      <span style={{ color: 'var(--ink-muted)', fontSize: 12, fontFamily: 'var(--mono)' }}>{file.items ? `${file.items}` : '—'}</span>
      <span style={{ color: 'var(--ink-muted)', fontSize: 12 }}>{file.mtime}</span>
      <span style={{ display: 'flex', justifyContent: 'flex-end', alignItems: 'center', gap: 8 }}>
        <StatusDot status={file.status} />
        {(h || selected) && (
          <button onClick={(e) => { e.stopPropagation(); onShare && onShare(); }}
            style={{ background: 'transparent', border: 'none', cursor: 'pointer', padding: 4, color: 'var(--ink-muted)', display: 'flex' }}>
            <Icon name="share" size={14} />
          </button>
        )}
      </span>
    </div>
  );
}

function FlatBtn({ icon, label, primary, onClick }) {
  const [h, setH] = useS(false);
  const style = primary ? {
    background: h ? 'var(--ink-2)' : 'var(--ink)', color: 'var(--cream)', border: 'none',
  } : {
    background: h ? 'var(--paper-2)' : 'transparent', color: 'var(--ink-soft)',
    border: '1px solid var(--hairline)',
  };
  return (
    <button onClick={onClick} onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{ ...style, padding: '7px 12px', borderRadius: 'var(--r-2)', display: 'flex', alignItems: 'center', gap: 7,
        fontSize: 12.5, cursor: 'pointer', fontWeight: 500 }}>
      <Icon name={icon} size={14} color={primary ? 'var(--cream)' : 'var(--ink-soft)'} />
      <span>{label}</span>
    </button>
  );
}

/* ─── Onboarding scene ─── */

function OnboardScene() {
  const [step, setStep] = useS(1);
  const [url, setUrl] = useS('https://cloud.sound.studio');
  const [folder, setFolder] = useS('~/Adagio');
  const steps = [
    { n: 1, name: 'Welcome' },
    { n: 2, name: 'Server' },
    { n: 3, name: 'Authorize' },
    { n: 4, name: 'Where to sync' },
    { n: 5, name: 'Begin' },
  ];

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
          {steps.map(s => {
            const done = step > s.n, active = step === s.n;
            return (
              <button key={s.n} onClick={() => setStep(s.n)} style={{
                background: 'transparent', border: 'none', textAlign: 'left', cursor: 'pointer',
                display: 'flex', alignItems: 'center', gap: 14, padding: '10px 0',
              }}>
                <div style={{ width: 22, height: 22, borderRadius: 11, display: 'flex', alignItems: 'center', justifyContent: 'center',
                  background: done ? 'var(--forest)' : active ? 'var(--clay)' : 'var(--cream-2)',
                  color: done || active ? 'var(--cream)' : 'var(--ink-muted)',
                  fontFamily: 'var(--mono)', fontSize: 11, fontWeight: 600, flexShrink: 0,
                }}>
                  {done ? <Icon name="check" size={10} color="var(--cream)" strokeWidth={3}/> : s.n}
                </div>
                <span style={{ fontSize: 13.5, color: active ? 'var(--ink)' : 'var(--ink-muted)', fontWeight: active ? 500 : 400 }}>{s.name}</span>
              </button>
            );
          })}
        </div>
      </div>

      {/* right canvas */}
      <div style={{ flex: 1, padding: '60px 80px', display: 'flex', flexDirection: 'column', justifyContent: 'space-between', overflow: 'auto' }}>
        {step === 1 && (
          <>
            <div>
              <div style={{ display: 'flex', alignItems: 'center', gap: 14, marginBottom: 28 }}>
                <MarkSlur size={48} fill="var(--ink)" accent="var(--clay)" />
                <AdagioWord size={56} />
              </div>
              <h1 style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 56, lineHeight: 1, margin: 0, letterSpacing: '-0.05em', maxWidth: 600 }}>
                Welcome.
              </h1>
              <p style={{ marginTop: 22, fontSize: 16, lineHeight: 1.6, color: 'var(--ink-soft)', maxWidth: 520 }}>
                Adagio is a desktop client for Nextcloud. It runs quietly in the background, keeps a single folder on your machine in sync, and stays out of your way the rest of the time.
              </p>
              <div style={{ marginTop: 32, display: 'flex', flexDirection: 'column', gap: 14 }}>
                {[
                  ['No telemetry, ever.', 'Adagio does not phone home. Crash logs stay on this machine until you choose to send them.'],
                  ['Files live in your filesystem.', 'No proprietary container, no virtual mount. Open them with anything.'],
                  ['Source open under AGPL-3.0.', 'You can read every line that touches your data.'],
                ].map(([h, b]) => (
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
            <OnboardFooter step={step} setStep={setStep} />
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
                <label style={{ fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.16em', textTransform: 'uppercase', color: 'var(--ink-muted)' }}>Server</label>
                <div style={{ marginTop: 8, display: 'flex', alignItems: 'center', gap: 0, background: 'var(--paper)', border: '1.5px solid var(--ink)', borderRadius: 'var(--r-2)', padding: '12px 14px' }}>
                  <Icon name="globe" size={16} color="var(--ink-muted)" />
                  <input value={url} onChange={e => setUrl(e.target.value)} style={{ flex: 1, border: 'none', background: 'transparent', outline: 'none', fontFamily: 'var(--mono)', fontSize: 14, color: 'var(--ink)', marginLeft: 10 }}/>
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
            <OnboardFooter step={step} setStep={setStep} />
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
                  <div style={{ marginTop: 12, fontSize: 12, opacity: 0.55 }}>expires in 04:21</div>
                </div>
                <div style={{ width: 100, height: 100, padding: 8, background: 'var(--cream)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)' }}>
                  <FakeQR />
                </div>
              </div>
              <div style={{ marginTop: 24, display: 'flex', alignItems: 'center', gap: 10, fontSize: 13, color: 'var(--ink-muted)' }}>
                <SpinDot color="var(--clay)" />
                <span>Waiting for confirmation at cloud.sound.studio…</span>
              </div>
            </div>
            <OnboardFooter step={step} setStep={setStep} />
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
                <label style={{ fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.16em', textTransform: 'uppercase', color: 'var(--ink-muted)' }}>Local folder</label>
                <div style={{ marginTop: 8, display: 'flex', alignItems: 'center', gap: 0, background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', padding: '12px 14px' }}>
                  <Icon name="folder" size={16} color="var(--ink-muted)" />
                  <input value={folder} onChange={e => setFolder(e.target.value)} style={{ flex: 1, border: 'none', background: 'transparent', outline: 'none', fontFamily: 'var(--mono)', fontSize: 14, marginLeft: 10 }}/>
                  <button style={{ background: 'var(--cream-2)', border: 'none', padding: '5px 10px', borderRadius: 'var(--r-1)', fontSize: 12, cursor: 'pointer' }}>Browse…</button>
                </div>
                <div style={{ marginTop: 16, display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
                  <ToggleRow title="On-demand" desc="Files exist as placeholders until opened. Saves disk." checked />
                  <ToggleRow title="Pin pinned folders" desc="Marked folders always live offline." checked />
                  <ToggleRow title="Smart bandwidth" desc="Throttles when on battery or hotspot." checked />
                  <ToggleRow title="Watch external edits" desc="Detect changes from other apps instantly." />
                </div>
              </div>
            </div>
            <OnboardFooter step={step} setStep={setStep} />
          </>
        )}

        {step === 5 && (
          <>
            <div style={{ marginTop: 40 }}>
              <div style={{ display: 'flex', alignItems: 'center', gap: 14, marginBottom: 28 }}>
                <Icon name="check-circ" size={28} color="var(--forest)" />
                <div style={{ fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: 'var(--forest)' }}>Connected to cloud.sound.studio</div>
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
            <OnboardFooter step={step} setStep={setStep} finish />
          </>
        )}
      </div>
    </div>
  );
}

function OnboardFooter({ step, setStep, finish }) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginTop: 40, paddingTop: 20, borderTop: '1px solid var(--hairline)' }}>
      <button onClick={() => setStep(Math.max(1, step - 1))} style={{ background: 'transparent', border: 'none', color: 'var(--ink-muted)', cursor: 'pointer', fontSize: 13, display: 'flex', alignItems: 'center', gap: 6 }}>
        ← Back
      </button>
      <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
        <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)', letterSpacing: '0.06em' }}>step {step} of 5</span>
        <button onClick={() => setStep(Math.min(5, step + 1))} style={{ background: 'var(--ink)', color: 'var(--cream)', border: 'none', padding: '10px 22px', borderRadius: 'var(--r-pill)', fontSize: 13, fontWeight: 500, cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 8 }}>
          {finish ? 'Open Adagio' : 'Continue'} <Icon name="arrow-r" size={14} color="var(--cream)" />
        </button>
      </div>
    </div>
  );
}

function ToggleRow({ title, desc, checked }) {
  const [on, setOn] = useS(!!checked);
  return (
    <button onClick={() => setOn(!on)} style={{ background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', padding: 14, textAlign: 'left', cursor: 'pointer', display: 'flex', flexDirection: 'column', gap: 4 }}>
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

function FakeQR() {
  // simple 9x9 deterministic-looking pattern
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

/* ─── Activity scene ─── */

function ActivityScene() {
  const [filter, setFilter] = useS('all');
  const filters = [['all', 'All', ACTIVITY.length], ['edits', 'Edits', 3], ['shares', 'Shares', 2], ['sync', 'Sync', 2], ['conflicts', 'Conflicts', 1]];

  const grouped = useMemo(() => {
    const byBucket = { 'Today': [], 'Yesterday': [], 'Earlier this week': [] };
    ACTIVITY.forEach((a, i) => {
      const k = i < 4 ? 'Today' : i < 7 ? 'Yesterday' : 'Earlier this week';
      byBucket[k].push(a);
    });
    return byBucket;
  }, []);

  return (
    <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden', background: 'var(--cream)' }}>
      <div style={{ height: 52, flexShrink: 0, padding: '0 22px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', borderBottom: '1px solid var(--hairline)' }}>
        <div style={{ display: 'flex', alignItems: 'baseline', gap: 14 }}>
          <span style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 20, letterSpacing: '-0.04em' }}>Activity</span>
          <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)', letterSpacing: '0.08em' }}>since you last checked in · 12 events</span>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 4, background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-pill)', padding: 3 }}>
          {filters.map(([id, lbl, count]) => (
            <button key={id} onClick={() => setFilter(id)} style={{
              background: filter === id ? 'var(--ink)' : 'transparent', color: filter === id ? 'var(--cream)' : 'var(--ink-soft)',
              border: 'none', padding: '5px 12px', borderRadius: 'var(--r-pill)', fontSize: 12, cursor: 'pointer', fontWeight: 500,
              display: 'flex', alignItems: 'center', gap: 6,
            }}>
              {lbl} <span style={{ fontFamily: 'var(--mono)', fontSize: 10, opacity: 0.6 }}>{count}</span>
            </button>
          ))}
        </div>
      </div>

      <div style={{ flex: 1, overflowY: 'auto', padding: '8px 0 24px' }}>
        {Object.entries(grouped).map(([bucket, items]) => (
          <div key={bucket}>
            <div style={{ padding: '20px 28px 10px', fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.22em', textTransform: 'uppercase', color: 'var(--ink-muted)' }}>{bucket}</div>
            {items.map((a, i) => <ActivityRow key={i} a={a} />)}
          </div>
        ))}
      </div>
    </div>
  );
}

function ActivityRow({ a }) {
  const tint = {
    edit: ['var(--clay)', 'edited'],
    share: ['var(--forest)', 'shared'],
    sync: ['var(--ink-soft)', 'synced'],
    conflict: ['var(--danger)', 'conflict'],
    pin: ['var(--ink)', 'pinned'],
    add: ['var(--forest)', 'added'],
    block: ['var(--ink-muted)', 'blocked'],
  }[a.bullet] || ['var(--ink-muted)', a.what];

  const [h, setH] = useS(false);
  return (
    <div onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{ display: 'flex', alignItems: 'center', gap: 16, padding: '14px 28px',
        background: h ? 'color-mix(in srgb, var(--ink) 4%, transparent)' : 'transparent', cursor: 'pointer',
        borderBottom: '1px solid var(--hairline-2)' }}>
      <div style={{ width: 32, height: 32, borderRadius: 16, background: 'var(--paper-2)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0,
        color: tint[0] }}>
        {a.bullet === 'edit' && <Icon name="pencil" size={14} color={tint[0]} />}
        {a.bullet === 'share' && <Icon name="share" size={14} color={tint[0]} />}
        {a.bullet === 'sync' && <Icon name="sync" size={14} color={tint[0]} />}
        {a.bullet === 'conflict' && <Icon name="warn" size={14} color={tint[0]} />}
        {a.bullet === 'pin' && <Icon name="pin" size={14} color={tint[0]} />}
        {a.bullet === 'add' && <Icon name="plus" size={14} color={tint[0]} />}
        {a.bullet === 'block' && <Icon name="x" size={14} color={tint[0]} />}
      </div>
      <div style={{ flex: 1, minWidth: 0 }}>
        <div style={{ fontSize: 14, color: 'var(--ink)' }}>
          <span style={{ fontWeight: 500 }}>{a.who}</span>{' '}
          <span style={{ color: 'var(--ink-muted)' }}>{a.what}</span>{' '}
          <span style={{ fontFamily: 'var(--body)', fontWeight: 500, letterSpacing: '-0.02em', fontSize: 14, color: 'var(--ink)' }}>{a.target}</span>
          {a.to && <span style={{ color: 'var(--ink-muted)' }}> · with {a.to}</span>}
        </div>
        <div style={{ fontSize: 11.5, color: 'var(--ink-muted)', fontFamily: 'var(--mono)', letterSpacing: '0.03em', marginTop: 3 }}>
          {a.where} · {a.when}
        </div>
      </div>
      {h && (
        <div style={{ display: 'flex', gap: 4 }}>
          <button style={{ background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 6, padding: '5px 10px', fontSize: 11.5, cursor: 'pointer', color: 'var(--ink-soft)' }}>Open</button>
          {a.bullet === 'conflict' && <button style={{ background: 'var(--danger)', color: 'var(--cream)', border: 'none', borderRadius: 6, padding: '5px 10px', fontSize: 11.5, cursor: 'pointer' }}>Resolve</button>}
        </div>
      )}
    </div>
  );
}

/* ─── Share dialog ─── */

function ShareDialog({ onClose }) {
  const [expiry, setExpiry] = useS('7d');
  const [perm, setPerm] = useS('view');
  const [pwd, setPwd] = useS(false);
  const [people, setPeople] = useS([{ name: 'Mira Olsson', role: 'editor' }, { name: 'Yui Tanaka', role: 'viewer' }]);
  return (
    <div style={{ position: 'absolute', inset: 0, background: 'color-mix(in srgb, var(--ink) 40%, transparent)', backdropFilter: 'blur(6px)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 20 }}>
      <div style={{ width: 540, background: 'var(--cream)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-3)', boxShadow: 'var(--shadow-lg)', overflow: 'hidden' }}>
        <div style={{ padding: '20px 24px 14px', display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', borderBottom: '1px solid var(--hairline)' }}>
          <div>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.2em', textTransform: 'uppercase', color: 'var(--clay)', marginBottom: 6 }}>Share</div>
            <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 22, lineHeight: 1.1, letterSpacing: '-0.035em' }}>Investor update Q2.md</div>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)', marginTop: 4 }}>14 KB · /Sound · last edited 4 min ago</div>
          </div>
          <button onClick={onClose} style={{ background: 'transparent', border: 'none', cursor: 'pointer', color: 'var(--ink-muted)' }}>
            <Icon name="x" size={18} />
          </button>
        </div>

        <div style={{ padding: '18px 24px', display: 'flex', flexDirection: 'column', gap: 16 }}>
          {/* who */}
          <div>
            <label style={shareLabel}>With</label>
            <div style={{ marginTop: 8, padding: '6px 8px 8px', background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)' }}>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, alignItems: 'center' }}>
                {people.map((p, i) => (
                  <span key={p.name} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 6px 4px 4px', background: 'var(--cream-2)', borderRadius: 'var(--r-pill)', fontSize: 12 }}>
                    <span style={{ width: 18, height: 18, borderRadius: 9, background: 'var(--forest)', color: 'var(--cream)', fontSize: 9, display: 'flex', alignItems: 'center', justifyContent: 'center', fontWeight: 600 }}>{p.name.split(' ').map(n => n[0]).join('')}</span>
                    {p.name}
                    <span style={{ color: 'var(--ink-muted)', fontFamily: 'var(--mono)', fontSize: 10, paddingLeft: 4, borderLeft: '1px solid var(--hairline)' }}>{p.role}</span>
                    <button onClick={() => setPeople(people.filter((_, j) => j !== i))} style={{ background: 'transparent', border: 'none', cursor: 'pointer', color: 'var(--ink-muted)', padding: 0, marginLeft: 2 }}>
                      <Icon name="x" size={12} />
                    </button>
                  </span>
                ))}
                <input placeholder="Add a name or email…" style={{ flex: 1, minWidth: 140, border: 'none', background: 'transparent', outline: 'none', fontSize: 13, padding: '4px 6px' }}/>
              </div>
            </div>
          </div>

          {/* permission */}
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 14 }}>
            <Segment label="Can" value={perm} setValue={setPerm} options={[['view', 'View'], ['comment', 'Comment'], ['edit', 'Edit']]} />
            <Segment label="Until" value={expiry} setValue={setExpiry} options={[['24h', '24 h'], ['7d', '7 days'], ['30d', '30 days'], ['never', 'No limit']]} />
          </div>

          {/* link */}
          <div>
            <label style={shareLabel}>Or, a link</label>
            <div style={{ marginTop: 8, display: 'flex', alignItems: 'center', gap: 8, background: 'var(--ink)', color: 'var(--cream)', padding: '10px 12px', borderRadius: 'var(--r-2)', fontFamily: 'var(--mono)', fontSize: 12 }}>
              <Icon name="link" size={14} color="var(--clay-soft)" />
              <span style={{ flex: 1, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>cloud.sound.studio/s/F4PS-9TRX-K2N</span>
              <button style={{ background: 'color-mix(in srgb, var(--cream) 12%, transparent)', color: 'var(--cream)', border: 'none', padding: '4px 10px', borderRadius: 4, fontSize: 11, cursor: 'pointer', fontFamily: 'inherit' }}>copy</button>
            </div>
            <div style={{ marginTop: 10, display: 'flex', alignItems: 'center', gap: 16, fontSize: 12.5, color: 'var(--ink-soft)' }}>
              <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer' }}>
                <input type="checkbox" checked={pwd} onChange={() => setPwd(!pwd)} style={{ accentColor: 'var(--ink)' }}/> Password protect
              </label>
              <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer' }}>
                <input type="checkbox" defaultChecked style={{ accentColor: 'var(--ink)' }}/> Hide download
              </label>
              <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer' }}>
                <input type="checkbox" style={{ accentColor: 'var(--ink)' }}/> Notify on open
              </label>
            </div>
          </div>

          {/* note */}
          <div>
            <label style={shareLabel}>A note (optional)</label>
            <textarea defaultValue="Latest numbers — happy to walk through on Thursday." style={{ marginTop: 8, width: '100%', minHeight: 56, padding: 12, border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', background: 'var(--paper)', fontFamily: 'var(--body)', fontSize: 13, color: 'var(--ink)', resize: 'none', outline: 'none', boxSizing: 'border-box' }}/>
          </div>
        </div>

        <div style={{ padding: '14px 24px', background: 'var(--paper-2)', borderTop: '1px solid var(--hairline)', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{ fontSize: 12, color: 'var(--ink-muted)', display: 'flex', alignItems: 'center', gap: 6 }}>
            <Icon name="shield" size={13} color="var(--forest)" /> End-to-end encrypted folder
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <button onClick={onClose} style={{ background: 'transparent', border: '1px solid var(--hairline)', padding: '8px 16px', borderRadius: 'var(--r-pill)', fontSize: 13, cursor: 'pointer', color: 'var(--ink)' }}>Cancel</button>
            <button style={{ background: 'var(--ink)', color: 'var(--cream)', border: 'none', padding: '8px 18px', borderRadius: 'var(--r-pill)', fontSize: 13, fontWeight: 500, cursor: 'pointer' }}>Share with 2 people</button>
          </div>
        </div>
      </div>
    </div>
  );
}

const shareLabel = { fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.18em', textTransform: 'uppercase', color: 'var(--ink-muted)' };

function Segment({ label, value, setValue, options }) {
  return (
    <div>
      <label style={shareLabel}>{label}</label>
      <div style={{ marginTop: 8, display: 'flex', background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', padding: 2 }}>
        {options.map(([id, lbl]) => (
          <button key={id} onClick={() => setValue(id)} style={{
            flex: 1, background: value === id ? 'var(--ink)' : 'transparent', color: value === id ? 'var(--cream)' : 'var(--ink-soft)',
            border: 'none', padding: '6px 8px', borderRadius: 4, fontSize: 12, fontWeight: 500, cursor: 'pointer',
          }}>{lbl}</button>
        ))}
      </div>
    </div>
  );
}

/* ─── Tray (menu bar) ─── */

function Tray() {
  return (
    <div style={{ width: '100%', height: '100%', padding: 12, background: 'var(--paper-2)', display: 'flex', alignItems: 'flex-start', justifyContent: 'center', fontFamily: 'var(--body)' }}>
      <div style={{ width: 380, background: 'var(--cream)', borderRadius: 'var(--r-3)', boxShadow: 'var(--shadow-lg)', overflow: 'hidden', border: '1px solid var(--hairline)' }}>
        {/* header */}
        <div style={{ padding: '14px 16px', display: 'flex', alignItems: 'center', gap: 10, borderBottom: '1px solid var(--hairline)' }}>
          <MarkSlur size={18} fill="var(--ink)" accent="var(--clay)" />
          <span style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 16, letterSpacing: '-0.05em' }}>adagio</span>
          <span style={{ marginLeft: 'auto', fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.14em', color: 'var(--good)', display: 'flex', alignItems: 'center', gap: 6 }}>
            <span style={{ width: 6, height: 6, borderRadius: 3, background: 'var(--good)' }}/> IN SYNC
          </span>
        </div>

        {/* status block */}
        <div style={{ padding: '14px 16px', borderBottom: '1px solid var(--hairline)' }}>
          <div style={{ display: 'flex', alignItems: 'flex-end', gap: 8, marginBottom: 4 }}>
            <span style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 28, lineHeight: 1, letterSpacing: '-0.045em' }}>Up to <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontWeight: 400, letterSpacing: '-0.01em' }}>date</span></span>
          </div>
          <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)', letterSpacing: '0.04em' }}>last sync 11:42 · 3,247 files · 86.7 GB</div>
        </div>

        {/* recent */}
        <div style={{ padding: '8px 16px' }}>
          <div style={{ padding: '6px 0', fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.18em', textTransform: 'uppercase', color: 'var(--ink-muted)' }}>Recent</div>
          {[
            { name: 'Investor update Q2.md', when: '4 min ago', ic: 'pencil' },
            { name: 'Logo · construction.svg', when: '12 min ago', ic: 'share' },
            { name: '7 files in Engineering/', when: '32 min ago', ic: 'sync' },
          ].map((r, i) => (
            <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 10, padding: '8px 0' }}>
              <div style={{ width: 24, height: 24, borderRadius: 12, background: 'var(--paper-2)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
                <Icon name={r.ic} size={12} color="var(--ink-soft)" />
              </div>
              <div style={{ flex: 1, minWidth: 0 }}>
                <div style={{ fontSize: 12.5, color: 'var(--ink)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{r.name}</div>
                <div style={{ fontSize: 11, color: 'var(--ink-muted)', fontFamily: 'var(--mono)' }}>{r.when}</div>
              </div>
            </div>
          ))}
        </div>

        {/* actions */}
        <div style={{ padding: 8, display: 'flex', flexDirection: 'column', gap: 1, borderTop: '1px solid var(--hairline)' }}>
          {[
            ['folder', 'Open Adagio folder', '⌘O'],
            ['globe', 'Open in browser', '⌘B'],
            ['sync', 'Pause syncing', '⌘P'],
            ['settings', 'Preferences…', '⌘,'],
          ].map(([ic, lbl, kbd]) => (
            <button key={lbl} style={{ background: 'transparent', border: 'none', display: 'flex', alignItems: 'center', gap: 12, padding: '8px 8px', borderRadius: 6, cursor: 'pointer', textAlign: 'left' }}
              onMouseEnter={(e) => e.currentTarget.style.background = 'var(--paper-2)'}
              onMouseLeave={(e) => e.currentTarget.style.background = 'transparent'}>
              <Icon name={ic} size={14} color="var(--ink-soft)" />
              <span style={{ flex: 1, fontSize: 13, color: 'var(--ink)' }}>{lbl}</span>
              <span style={{ fontFamily: 'var(--mono)', fontSize: 10.5, color: 'var(--ink-muted)' }}>{kbd}</span>
            </button>
          ))}
        </div>

        <div style={{ padding: '10px 16px', borderTop: '1px solid var(--hairline)', background: 'var(--paper)', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
          <span style={{ fontFamily: 'var(--mono)', fontSize: 10.5, color: 'var(--ink-muted)', letterSpacing: '0.04em' }}>v1.0 · allegretto</span>
          <span style={{ fontFamily: 'var(--mono)', fontSize: 10.5, color: 'var(--ink-muted)', cursor: 'pointer' }}>Quit Adagio</span>
        </div>
      </div>
    </div>
  );
}

/* ─── main dispatcher ─── */

function DesktopApp({ scene = 'files', miniature }) {
  const [tab, setTab] = useS(scene === 'activity' ? 'activity' : 'files');
  const [account] = useS(ACCOUNTS[0]);
  const [source, setSource] = useS('all');
  const [shareOpen, setShareOpen] = useS(scene === 'share');

  // onboarding has no chrome/sidebar
  if (scene === 'onboard') {
    return (
      <Chrome>
        <OnboardScene />
      </Chrome>
    );
  }

  const activeTab = scene === 'activity' ? 'activity' : tab;
  return (
    <div style={{ width: '100%', height: '100%', position: 'relative', borderRadius: 'inherit', overflow: 'hidden' }}>
      <Chrome tab={activeTab} onTab={setTab}>
        <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
          <Sidebar selected={source} onSelect={setSource} account={account} />
          {activeTab === 'files' ? (
            <FilesScene files={FILE_TREE['/Sound']} onShare={() => setShareOpen(true)} sourceFilter={source} miniature={miniature} />
          ) : (
            <ActivityScene />
          )}
        </div>
      </Chrome>
      {shareOpen && <ShareDialog onClose={() => setShareOpen(false)} />}
    </div>
  );
}

Object.assign(window, { DesktopApp, Tray });
