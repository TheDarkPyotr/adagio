/* website.jsx — landing page (modernized v2) */

function Website() {
  const [tab, setTab] = React.useState('linux');
  return (
    <div style={{ width: '100%', height: '100%', background: 'var(--cream)', overflow: 'hidden', color: 'var(--ink)', fontFamily: 'var(--body)' }}>
      <WebAnnounce />
      <WebNav />
      <WebHero />
      <WebTicker />
      <WebTrust />
      <WebQuote />
      <WebSpecimen />
      <WebLeadFeature />
      <WebFeatures />
      <WebManifesto />
      <WebComposition />
      <WebDownload tab={tab} setTab={setTab} />
      <WebFooter />
    </div>
  );
}

/* ── shared utilities ─────────────────────────────────── */

const navLink = { color: 'inherit', textDecoration: 'none', cursor: 'pointer', fontWeight: 500 };

const btnPrimary = {
  background: 'var(--ink)', color: 'var(--cream)', border: 'none', padding: '11px 18px',
  borderRadius: 'var(--r-2)', fontSize: 13, fontWeight: 500, cursor: 'pointer', letterSpacing: '-0.005em',
  display: 'inline-flex', alignItems: 'center', gap: 8,
};
const btnGhost = {
  background: 'transparent', color: 'var(--ink)', border: '1px solid var(--ink)', padding: '10px 17px',
  borderRadius: 'var(--r-2)', fontSize: 13, fontWeight: 500, cursor: 'pointer',
  display: 'inline-flex', alignItems: 'center', gap: 8,
};

function Eyebrow({ children, accent = 'var(--clay)', light }) {
  return (
    <div style={{
      display: 'inline-flex', alignItems: 'center', gap: 8,
      fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '0.06em', textTransform: 'uppercase',
      color: light ? 'color-mix(in srgb, var(--cream) 60%, transparent)' : 'var(--ink-muted)',
    }}>
      <span style={{ width: 6, height: 6, borderRadius: 1, background: accent }}/>
      <span>{children}</span>
    </div>
  );
}

function Mono({ children, style }) {
  return <span style={{ fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '0.04em', color: 'var(--ink-muted)', ...style }}>{children}</span>;
}

function SectionRule({ light }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
      <span style={{ flex: 1, height: 1, background: light ? 'color-mix(in srgb, var(--cream) 14%, transparent)' : 'var(--hairline)' }}/>
      <MarkSlur size={14} fill={light ? 'var(--cream)' : 'var(--ink)'} accent="var(--clay)" modern />
      <span style={{ flex: 1, height: 1, background: light ? 'color-mix(in srgb, var(--cream) 14%, transparent)' : 'var(--hairline)' }}/>
    </div>
  );
}

/* ── sections ─────────────────────────────────────────── */

function WebAnnounce() {
  return (
    <div style={{ background: 'var(--ink)', color: 'var(--cream)', padding: '10px 56px', display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '0.04em' }}>
      <span style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
        <span style={{ display: 'inline-flex', padding: '2px 7px', background: 'var(--clay)', color: 'var(--cream)', borderRadius: 3, fontSize: 10, fontWeight: 600, letterSpacing: '0.08em' }}>NEW</span>
        <span>v1.0 · Allegretto · released may 2026</span>
      </span>
      <span style={{ display: 'flex', alignItems: 'center', gap: 28 }}>
        <span style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span style={{ width: 6, height: 6, borderRadius: 3, background: 'var(--clay-soft)', animation: 'pulse 1.8s ease-in-out infinite' }}/>
          <span style={{ opacity: 0.7 }}>3,247 servers in sync</span>
        </span>
        <span style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          Release notes <span style={{ opacity: 0.55 }}>→</span>
        </span>
      </span>
      <style>{`@keyframes pulse { 0%,100% { opacity: 1 } 50% { opacity: .35 } }`}</style>
    </div>
  );
}

function WebNav() {
  return (
    <div style={{ padding: '20px 56px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', borderBottom: '1px solid var(--hairline)', position: 'relative' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <MarkSlur size={24} fill="var(--ink)" accent="var(--clay)" modern />
        <AdagioWord size={24} weight={500} />
        <span style={{ marginLeft: 10, padding: '2px 8px', background: 'var(--paper-2)', borderRadius: 3, fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--ink-muted)', letterSpacing: '0.04em' }}>for nextcloud</span>
      </div>
      <div style={{ position: 'absolute', left: '50%', transform: 'translateX(-50%)', display: 'flex', alignItems: 'center', gap: 32, fontSize: 14, color: 'var(--ink-soft)' }}>
        <a style={navLink}>Features</a>
        <a style={navLink}>Composition</a>
        <a style={navLink}>Self-host</a>
        <a style={navLink}>Changelog</a>
        <a style={navLink}>Docs</a>
      </div>
      <div style={{ display: 'flex', alignItems: 'center', gap: 12 }}>
        <a style={{ ...navLink, color: 'var(--ink)' }}>Sign in</a>
        <button style={btnPrimary}>
          Download <span style={{ opacity: 0.55, fontFamily: 'var(--mono)', fontSize: 11 }}>4.2 MB</span>
        </button>
      </div>
    </div>
  );
}

function WebHero() {
  return (
    <div style={{ padding: '80px 56px 96px', display: 'grid', gridTemplateColumns: '2fr 1fr', gap: 56, alignItems: 'center', position: 'relative' }}>
      <div>
        <Eyebrow>A desktop client for Nextcloud</Eyebrow>
        <h1 style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 100, lineHeight: 0.98, margin: '28px 0 0', letterSpacing: '-0.055em', color: 'var(--ink)' }}>
          The unhurried <br/>way to keep your <br/>files <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontWeight: 400, letterSpacing: '-0.02em' }}>in sync.</span>
        </h1>
        <p style={{ marginTop: 36, fontSize: 19, lineHeight: 1.55, color: 'var(--ink-soft)', maxWidth: 540, fontWeight: 400 }}>
          Adagio is a desktop client for Nextcloud — quiet, deliberate, out of your way. No dashboards. No popups. Just your files, kept honest in the background.
        </p>
        <div style={{ marginTop: 40, display: 'flex', alignItems: 'center', gap: 12 }}>
          <button style={{ ...btnPrimary, padding: '14px 22px', fontSize: 14 }}>
            <Icon name="cloud-dl" size={15} color="var(--cream)" />Download for Linux
            <span style={{ opacity: 0.55, fontFamily: 'var(--mono)', fontSize: 11 }}>· 4.2 MB</span>
          </button>
          <button style={{ ...btnGhost, padding: '13px 21px', fontSize: 14 }}>See it in motion ↓</button>
        </div>
        <div style={{ marginTop: 28, display: 'flex', gap: 14, alignItems: 'center', flexWrap: 'wrap' }}>
          <Mono>macOS 12+</Mono>
          <Mono style={{ opacity: 0.4 }}>·</Mono>
          <Mono>Windows 10+</Mono>
          <Mono style={{ opacity: 0.4 }}>·</Mono>
          <Mono>deb / rpm / flatpak</Mono>
        </div>
      </div>

      {/* hero artwork: oversized slur, with a floating sync card */}
      <div style={{ position: 'relative', height: 560 }}>
        <div style={{ position: 'absolute', inset: 0, display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
          <svg width="500" height="500" viewBox="0 0 500 500" fill="none">
            {/* construction grid */}
            <line x1="0" y1="250" x2="500" y2="250" stroke="var(--hairline)" strokeWidth="1"/>
            <line x1="250" y1="0" x2="250" y2="500" stroke="var(--hairline)" strokeWidth="1"/>
            <circle cx="250" cy="250" r="180" stroke="var(--hairline)" strokeWidth="1" fill="none" strokeDasharray="3 4"/>

            {/* the mark */}
            <path d="M70 360 C 120 80, 380 80, 430 360" stroke="var(--ink)" strokeWidth="16" strokeLinecap="round" fill="none"/>
            <circle cx="70" cy="360" r="42" fill="var(--ink)"/>
            <circle cx="430" cy="360" r="42" fill="var(--clay)"/>
          </svg>
        </div>

        {/* floating sync card (top-right) */}
        <div style={{ position: 'absolute', top: 30, right: 10, padding: '10px 14px', background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', boxShadow: 'var(--shadow-md)', display: 'flex', alignItems: 'center', gap: 10, fontSize: 12 }}>
          <span style={{ width: 14, height: 14, position: 'relative' }}>
            <svg width="14" height="14" viewBox="0 0 14 14" style={{ animation: 'adagio-spin 1.4s linear infinite' }}>
              <circle cx="7" cy="7" r="5.5" stroke="var(--clay)" strokeOpacity="0.25" strokeWidth="1.6" fill="none"/>
              <path d="M7 1.5a5.5 5.5 0 0 1 5.5 5.5" stroke="var(--clay)" strokeWidth="1.6" strokeLinecap="round" fill="none"/>
            </svg>
          </span>
          <span style={{ color: 'var(--ink)' }}>Syncing</span>
          <Mono>14.6 MB · 38s</Mono>
          <style>{`@keyframes adagio-spin{to{transform:rotate(360deg)}}`}</style>
        </div>

        {/* floating "in sync" pill (bottom-left) */}
        <div style={{ position: 'absolute', bottom: 50, left: 0, padding: '8px 14px', background: 'var(--ink)', color: 'var(--cream)', borderRadius: 'var(--r-pill)', boxShadow: 'var(--shadow-md)', display: 'flex', alignItems: 'center', gap: 8, fontSize: 11, fontFamily: 'var(--mono)', letterSpacing: '0.08em' }}>
          <span style={{ width: 6, height: 6, borderRadius: 3, background: 'var(--good)' }}/>
          IN SYNC · 11:42
        </div>

        {/* annotation */}
        <div style={{ position: 'absolute', top: 30, left: 0, fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--ink-muted)', lineHeight: 1.6, letterSpacing: '0.03em' }}>
          fig 01 / the mark<br/>
          <span style={{ opacity: 0.5 }}>500 × 500 / stroke 16</span>
        </div>
      </div>
    </div>
  );
}

/* live-sync ticker — feels like a status bar at scale */
function WebTicker() {
  const items = [
    ['edit', 'Mira', 'Investor update Q2.md', '4m'],
    ['sync', 'Adagio', 'pulled 7 files from /Engineering', '12m'],
    ['share', 'You', 'Logo · construction.svg → Yui', '18m'],
    ['add',  'Felix', 'Composition.fig', '24m'],
    ['conflict', 'Conflict', 'Manifesto — draft.txt', '1h'],
    ['edit', 'Mira', 'Adagio · style.css', '2h'],
    ['pin',  'You', 'Engineering', '3h'],
  ];
  const tintFor = b => ({
    edit: 'var(--clay)', share: 'var(--forest)', sync: 'var(--ink-soft)', add: 'var(--forest)', pin: 'var(--ink)', conflict: 'var(--danger)',
  }[b] || 'var(--ink-muted)');
  return (
    <div style={{ borderTop: '1px solid var(--hairline)', borderBottom: '1px solid var(--hairline)', background: 'var(--paper)', overflow: 'hidden' }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 0, padding: '12px 0' }}>
        <div style={{ padding: '0 24px 0 56px', flexShrink: 0, borderRight: '1px solid var(--hairline)', display: 'flex', alignItems: 'center', gap: 8 }}>
          <span style={{ width: 6, height: 6, borderRadius: 3, background: 'var(--clay)' }}/>
          <Mono style={{ color: 'var(--ink)', fontWeight: 500 }}>LIVE · ACTIVITY</Mono>
        </div>
        <div style={{ flex: 1, display: 'flex', alignItems: 'center', gap: 32, padding: '0 24px', overflow: 'hidden', whiteSpace: 'nowrap' }}>
          {items.map((it, i) => (
            <div key={i} style={{ display: 'flex', alignItems: 'center', gap: 8, fontSize: 12.5, flexShrink: 0 }}>
              <span style={{ width: 5, height: 5, borderRadius: 2.5, background: tintFor(it[0]) }}/>
              <span style={{ color: 'var(--ink)', fontWeight: 500 }}>{it[1]}</span>
              <span style={{ color: 'var(--ink-muted)' }}>{it[0]}ed</span>
              <span style={{ color: 'var(--ink)' }}>{it[2]}</span>
              <Mono>{it[3]}</Mono>
            </div>
          ))}
        </div>
        <div style={{ padding: '0 56px 0 24px', flexShrink: 0, borderLeft: '1px solid var(--hairline)' }}>
          <Mono>cloud.sound.studio</Mono>
        </div>
      </div>
    </div>
  );
}

function WebTrust() {
  const orgs = ['MURENA', 'HETZNER', 'IONOS', 'NEXTCLOUD', 'KOMPETENZWERK', 'CITRIX', 'DISROOT'];
  return (
    <div style={{ padding: '64px 56px', borderBottom: '1px solid var(--hairline)' }}>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 2.5fr', gap: 56, alignItems: 'center' }}>
        <div>
          <Mono style={{ color: 'var(--ink)' }}>Tested against the providers your team actually uses —</Mono>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', gap: 36, flexWrap: 'wrap' }}>
          {orgs.map(o => (
            <span key={o} style={{ fontFamily: 'var(--mono)', fontSize: 13, letterSpacing: '0.18em', color: 'var(--ink-muted)', fontWeight: 500 }}>{o}</span>
          ))}
        </div>
      </div>
    </div>
  );
}

function WebQuote() {
  return (
    <div style={{ padding: '120px 56px', background: 'var(--paper-2)', borderBottom: '1px solid var(--hairline)', position: 'relative' }}>
      <div style={{ maxWidth: 1080, margin: '0 auto', display: 'grid', gridTemplateColumns: '1fr 4fr', gap: 56, alignItems: 'start' }}>
        <div>
          <Eyebrow>§ 01 · What it is</Eyebrow>
          <div style={{ marginTop: 16, fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--ink-muted)' }}>
            philosophy<br/>position<br/>tempo
          </div>
        </div>
        <p style={{ fontFamily: 'var(--body)', fontWeight: 400, fontSize: 48, lineHeight: 1.16, margin: 0, letterSpacing: '-0.03em', color: 'var(--ink)', textWrap: 'pretty' }}>
          The official Nextcloud client treats sync as software.
          <br/>
          <span style={{ color: 'var(--ink-muted)' }}>Adagio treats it as a piece of furniture — you don't think about it; it's just there, holding your work.</span>
        </p>
      </div>
    </div>
  );
}

function WebSpecimen() {
  return (
    <div style={{ padding: '112px 56px', borderBottom: '1px solid var(--hairline)' }}>
      <div style={{ display: 'flex', alignItems: 'flex-end', justifyContent: 'space-between', marginBottom: 48 }}>
        <div>
          <Eyebrow>§ 02 · The client</Eyebrow>
          <h2 style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 72, lineHeight: 1, margin: '20px 0 0', letterSpacing: '-0.045em' }}>
            A workspace, <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontWeight: 400, letterSpacing: '-0.01em' }}>not</span> a dashboard.
          </h2>
        </div>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 8, alignItems: 'flex-end' }}>
          <p style={{ fontSize: 16, lineHeight: 1.6, color: 'var(--ink-soft)', maxWidth: 380, margin: 0 }}>
            Adagio sits between your file manager and your server. It doesn't try to replace either — it keeps them in tempo, and surfaces what's worth your attention.
          </p>
          <div style={{ display: 'flex', gap: 6, marginTop: 12 }}>
            {['Files', 'Activity', 'Share', 'Tray'].map((t, i) => (
              <span key={t} style={{ padding: '5px 11px', background: i === 0 ? 'var(--ink)' : 'var(--paper)', color: i === 0 ? 'var(--cream)' : 'var(--ink-soft)', border: '1px solid', borderColor: i === 0 ? 'var(--ink)' : 'var(--hairline)', borderRadius: 'var(--r-pill)', fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.06em' }}>{t}</span>
            ))}
          </div>
        </div>
      </div>

      {/* specimen frame, with browser-like top */}
      <div style={{ background: 'var(--cream-2)', padding: 28, borderRadius: 'var(--r-3)', border: '1px solid var(--hairline)', position: 'relative' }}>
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 16, padding: '0 4px' }}>
          <div style={{ display: 'flex', gap: 6 }}>
            <span style={{ width: 8, height: 8, borderRadius: 4, background: 'var(--ink-muted)', opacity: 0.4 }}/>
            <span style={{ width: 8, height: 8, borderRadius: 4, background: 'var(--ink-muted)', opacity: 0.4 }}/>
            <span style={{ width: 8, height: 8, borderRadius: 4, background: 'var(--ink-muted)', opacity: 0.4 }}/>
          </div>
          <Mono>adagio · main browser</Mono>
          <Mono>linux · 1280 × 800</Mono>
        </div>
        <div style={{ width: '100%', aspectRatio: '16 / 10', background: 'var(--paper)', borderRadius: 8, boxShadow: 'var(--shadow-lg)', overflow: 'hidden', position: 'relative' }}>
          <DesktopApp scene="files" miniature />
        </div>
      </div>

      {/* annotations under the specimen */}
      <div style={{ marginTop: 32, display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 24 }}>
        {[
          ['A', 'Account switcher', 'work / personal · always two clicks away'],
          ['B', 'On-demand placeholders', 'cloud icon = on server · check = local'],
          ['C', 'Live activity rail', 'sync state lives in the chrome, not a popup'],
          ['D', 'Editorial breadcrumb', 'where you are, in serif — never lost'],
        ].map(([k, h, b]) => (
          <div key={k} style={{ paddingTop: 12, borderTop: '1px solid var(--hairline)' }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <span style={{ width: 18, height: 18, borderRadius: 9, background: 'var(--ink)', color: 'var(--cream)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontFamily: 'var(--mono)', fontSize: 9, fontWeight: 600 }}>{k}</span>
              <span style={{ fontSize: 13, fontWeight: 500 }}>{h}</span>
            </div>
            <p style={{ marginTop: 6, fontSize: 12.5, lineHeight: 1.5, color: 'var(--ink-soft)' }}>{b}</p>
          </div>
        ))}
      </div>
    </div>
  );
}

/* the lead feature — a big, dramatic intro to the feature grid */
function WebLeadFeature() {
  return (
    <div style={{ padding: '128px 56px 32px', borderBottom: '1px solid var(--hairline)' }}>
      <div style={{ marginBottom: 56 }}>
        <Eyebrow>§ 03 · Features</Eyebrow>
        <h2 style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 92, lineHeight: 0.98, margin: '20px 0 0', letterSpacing: '-0.05em', maxWidth: 1000 }}>
          Composed from six{' '}
          <span style={{ color: 'var(--ink-muted)' }}>quiet ideas.</span>
        </h2>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1.1fr', gap: 64, alignItems: 'center', padding: '40px 0', borderTop: '1px solid var(--hairline)' }}>
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 28 }}>
            <span style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--ink-muted)' }}>01</span>
            <span style={{ width: 28, height: 1, background: 'var(--hairline)' }}/>
            <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--clay)' }}>SYNC ENGINE</span>
          </div>
          <h3 style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 56, lineHeight: 1.02, letterSpacing: '-0.04em', margin: '0 0 24px' }}>
            In tempo, <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontWeight: 400 }}>never</span> out of breath.
          </h3>
          <p style={{ fontSize: 17, lineHeight: 1.6, color: 'var(--ink-soft)', maxWidth: 480, margin: 0 }}>
            A new scheduler that respects your laptop. Smooth, throttled transfers; no fan spin-up when you open a 4 GB folder. Cadence — written in Rust — does the work in 62 MB of RAM.
          </p>
          <div style={{ marginTop: 36, display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 24 }}>
            {[['62MB', 'idle RAM'], ['4.2MB', 'binary size'], ['~0%', 'CPU at rest']].map(([n, l]) => (
              <div key={l}>
                <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 38, letterSpacing: '-0.04em', color: 'var(--ink)' }}>{n}</div>
                <Mono style={{ marginTop: 2 }}>{l}</Mono>
              </div>
            ))}
          </div>
        </div>

        {/* tempo waveform visualization */}
        <div style={{ background: 'var(--paper-2)', borderRadius: 'var(--r-3)', border: '1px solid var(--hairline)', padding: 32, height: 360, position: 'relative', overflow: 'hidden' }}>
          <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-start', marginBottom: 24 }}>
            <Mono style={{ color: 'var(--ink)' }}>cadence · throughput</Mono>
            <Mono>last 60 minutes</Mono>
          </div>
          <svg width="100%" height="220" viewBox="0 0 480 220" preserveAspectRatio="none">
            {/* grid lines */}
            {[60, 110, 160].map(y => (
              <line key={y} x1="0" y1={y} x2="480" y2={y} stroke="var(--hairline)" strokeWidth="1" strokeDasharray="2 3"/>
            ))}
            {/* the tempo line — calm, regular, slow rhythm */}
            <path d="M0 110 Q 30 110, 30 110 Q 60 110, 60 90 Q 90 90, 90 110 Q 130 110, 130 70 Q 170 70, 170 110 Q 200 110, 200 110 Q 240 110, 240 95 Q 270 95, 270 110 Q 310 110, 310 60 Q 350 60, 350 110 Q 400 110, 400 110 Q 440 110, 440 100 Q 480 100, 480 110" stroke="var(--ink)" strokeWidth="2" fill="none"/>
            {/* clay accent bars representing transfers */}
            {[60, 130, 200, 270, 310, 400].map((x, i) => (
              <rect key={i} x={x - 2} y={110} width="4" height={Math.abs(Math.sin(i) * 30) + 8} fill="var(--clay)" opacity="0.85"/>
            ))}
          </svg>
          <div style={{ position: 'absolute', bottom: 24, left: 32, right: 32, display: 'flex', justifyContent: 'space-between' }}>
            <Mono>-60m</Mono>
            <Mono>-30m</Mono>
            <Mono style={{ color: 'var(--ink)', fontWeight: 500 }}>now</Mono>
          </div>
        </div>
      </div>
    </div>
  );
}

function WebFeatures() {
  const items = [
    { tag: 'Files',     title: 'On-demand by default.',       body: 'Every file is one click from local. Nothing eats your disk until you ask. Pin folders to keep them offline.', stat: 'Saves 86%', statLabel: 'disk on first run' },
    { tag: 'Activity',  title: 'A calm feed, not a firehose.',body: 'See what changed since you stepped away, grouped by person and project. Mute folders you don\'t need to watch.', stat: '12 events', statLabel: 'avg / hour' },
    { tag: 'Sharing',   title: 'A share is a sentence.',      body: 'One panel. Pick who, choose how long, add a note. Links expire when they should.', stat: '1 panel', statLabel: 'not seven dialogs' },
    { tag: 'Conflicts', title: 'Resolution without panic.',   body: 'Side-by-side diffs for anything text. For binaries, both versions land in the folder — you decide.', stat: 'No data loss', statLabel: 'audited Cure53' },
    { tag: 'Privacy',   title: 'End-to-end where it matters.',body: 'Per-folder E2EE using your server\'s keys. Adagio never sees your content. Your provider doesn\'t either.', stat: 'Zero', statLabel: 'telemetry · ever' },
  ];
  return (
    <div style={{ padding: '0 56px 112px', borderBottom: '1px solid var(--hairline)' }}>
      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 0, borderTop: '1px solid var(--hairline)' }}>
        {items.map((it, i) => (
          <div key={i} style={{
            padding: '40px 32px 40px 0',
            borderRight: (i % 3 !== 2) ? '1px solid var(--hairline)' : 'none',
            borderBottom: i < items.length - 3 || (items.length % 3 !== 0 && i < items.length - (items.length % 3)) ? '1px solid var(--hairline)' : 'none',
            paddingLeft: i % 3 !== 0 ? 32 : 0,
          }}>
            <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginBottom: 22 }}>
              <span style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--ink-muted)' }}>0{i + 2}</span>
              <span style={{ width: 18, height: 1, background: 'var(--hairline)' }}/>
              <span style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--clay)' }}>{it.tag}</span>
            </div>
            <h3 style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 28, lineHeight: 1.1, letterSpacing: '-0.035em', margin: '0 0 12px' }}>{it.title}</h3>
            <p style={{ fontSize: 15, lineHeight: 1.6, color: 'var(--ink-soft)', margin: 0 }}>{it.body}</p>
            <div style={{ marginTop: 28, paddingTop: 16, borderTop: '1px solid var(--hairline-2)', display: 'flex', alignItems: 'baseline', gap: 10 }}>
              <span style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 20, letterSpacing: '-0.03em', color: 'var(--ink)' }}>{it.stat}</span>
              <Mono>{it.statLabel}</Mono>
            </div>
          </div>
        ))}
        {/* one extra cell — a "more" pointer */}
        <div style={{
          padding: '40px 0 40px 32px',
          background: 'var(--paper-2)',
          margin: '0 -32px 0 0',
          display: 'flex', flexDirection: 'column', justifyContent: 'space-between',
        }}>
          <Mono style={{ color: 'var(--ink)' }}>+ 14 more</Mono>
          <div>
            <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 28, lineHeight: 1.1, letterSpacing: '-0.035em', marginBottom: 12 }}>
              The full <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic' }}>repertoire.</span>
            </div>
            <a style={{ ...navLink, fontSize: 13, color: 'var(--ink)', display: 'inline-flex', alignItems: 'center', gap: 6 }}>
              Read the feature list <Icon name="arrow-r" size={13} color="var(--ink)" />
            </a>
          </div>
        </div>
      </div>
    </div>
  );
}

function WebManifesto() {
  return (
    <div style={{ padding: '120px 56px', background: 'var(--cream)', borderBottom: '1px solid var(--hairline)' }}>
      <div style={{ maxWidth: 980, margin: '0 auto' }}>
        <SectionRule />
        <div style={{ textAlign: 'center', marginTop: 48 }}>
          <Eyebrow>§ 04 · Manifesto</Eyebrow>
          <p style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontSize: 56, lineHeight: 1.25, margin: '24px 0 0', letterSpacing: '-0.015em', color: 'var(--ink)', textWrap: 'balance' }}>
            "Software should be felt the way a doorframe is felt. <br/>
            <span style={{ color: 'var(--ink-muted)' }}>You move through it. You don't stop to admire it. <br/>It just lets you pass."</span>
          </p>
          <div style={{ marginTop: 36, display: 'inline-flex', alignItems: 'center', gap: 12 }}>
            <div style={{ width: 36, height: 36, borderRadius: 18, background: 'var(--forest)', color: 'var(--cream)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontFamily: 'var(--body)', fontWeight: 600 }}>L</div>
            <div style={{ textAlign: 'left' }}>
              <div style={{ fontSize: 14, fontWeight: 500 }}>Lena Hartmann</div>
              <Mono>founder · sound</Mono>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

function WebComposition() {
  const rows = [
    ['Compatible with',  'Nextcloud 27+, all auth flows'],
    ['Sync engine',      'Adagio Cadence · written in Rust'],
    ['Memory at idle',   '~62 MB'],
    ['On disk',          '4.2 MB · single binary'],
    ['Source',           'AGPL-3.0 · github.com/adagio'],
    ['Telemetry',        'None. By design.'],
    ['Audited',          'Cure53 · April 2026'],
    ['Tested against',   'Murena, Hetzner, IONOS, self-hosted'],
  ];
  return (
    <div style={{ padding: '128px 56px', background: 'var(--ink)', color: 'var(--cream)' }}>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1.4fr', gap: 80, alignItems: 'start' }}>
        <div>
          <Eyebrow accent="var(--clay-soft)" light>§ 05 · Composition</Eyebrow>
          <h2 style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 72, lineHeight: 1, margin: '24px 0 0', letterSpacing: '-0.045em' }}>
            What it's <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontWeight: 400, letterSpacing: '-0.01em' }}>made of.</span>
          </h2>
          <p style={{ marginTop: 24, fontSize: 16, lineHeight: 1.65, color: 'color-mix(in srgb, var(--cream) 65%, transparent)', maxWidth: 380 }}>
            Adagio is small on purpose. The whole client fits in a single binary you can read end to end in an afternoon.
          </p>

          {/* a small "open source" callout */}
          <div style={{ marginTop: 36, padding: 20, border: '1px solid color-mix(in srgb, var(--cream) 16%, transparent)', borderRadius: 'var(--r-2)' }}>
            <Mono style={{ color: 'color-mix(in srgb, var(--cream) 55%, transparent)' }}>$ git clone</Mono>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 14, color: 'var(--cream)', marginTop: 6 }}>github.com/adagio/adagio</div>
            <div style={{ marginTop: 14, display: 'flex', gap: 18, fontSize: 11, color: 'color-mix(in srgb, var(--cream) 55%, transparent)', fontFamily: 'var(--mono)' }}>
              <span>★ 4.2k</span>
              <span>⑂ 318</span>
              <span>AGPL-3.0</span>
            </div>
          </div>
        </div>

        <div>
          {rows.map(([k, v], i) => (
            <div key={i} style={{
              display: 'grid', gridTemplateColumns: '0.6fr 0.1fr 2fr', alignItems: 'baseline', gap: 0,
              padding: '22px 0',
              borderTop: '1px solid color-mix(in srgb, var(--cream) 10%, transparent)',
              borderBottom: i === rows.length - 1 ? '1px solid color-mix(in srgb, var(--cream) 10%, transparent)' : 'none',
            }}>
              <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'color-mix(in srgb, var(--cream) 55%, transparent)' }}>{k}</div>
              <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'color-mix(in srgb, var(--cream) 30%, transparent)' }}>0{i + 1}</div>
              <div style={{ fontFamily: 'var(--body)', fontWeight: 400, fontSize: 22, letterSpacing: '-0.02em' }}>{v}</div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

function WebDownload({ tab, setTab }) {
  const tabs = [
    { id: 'linux', label: 'Linux',   sub: 'deb · rpm · flatpak · appimage', icon: <PenguinGlyph /> },
    { id: 'mac',   label: 'macOS',   sub: 'universal · 12 Monterey+',        icon: <AppleGlyph /> },
    { id: 'win',   label: 'Windows', sub: 'msi · winget · scoop',            icon: <WindowsGlyph /> },
  ];
  const installs = {
    linux: ['flatpak install flathub so.adagio.Adagio', 'apt install adagio', 'dnf install adagio', 'curl https://thedarkpyotr.github.io/adagio/install | sh'],
    mac:   ['brew install --cask adagio', 'curl https://thedarkpyotr.github.io/adagio/install | sh'],
    win:   ['winget install Adagio.Adagio', 'scoop install adagio'],
  };
  const files = {
    linux: ['adagio_1.0.0_amd64.deb', '4.2 MB'],
    mac:   ['Adagio-1.0.0.dmg', '9.8 MB'],
    win:   ['Adagio-1.0.0.msi', '12.1 MB'],
  };

  return (
    <div style={{ padding: '128px 56px', background: 'var(--cream)', borderBottom: '1px solid var(--hairline)' }}>
      <div style={{ textAlign: 'center', marginBottom: 56, display: 'flex', flexDirection: 'column', alignItems: 'center' }}>
        <Eyebrow>§ 06 · Download · v1.0</Eyebrow>
        <h2 style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 96, lineHeight: 0.95, margin: '20px 0 0', letterSpacing: '-0.05em' }}>
          Begin <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontWeight: 400, letterSpacing: '-0.01em' }}>softly.</span>
        </h2>
        <p style={{ marginTop: 24, fontSize: 17, lineHeight: 1.6, color: 'var(--ink-soft)', maxWidth: 540 }}>
          One binary. No installer ceremony. Connect to your Nextcloud and Adagio takes care of the rest.
        </p>
      </div>

      <div style={{ maxWidth: 920, margin: '0 auto' }}>
        <div style={{ display: 'flex', gap: 0, borderBottom: '1px solid var(--hairline)' }}>
          {tabs.map(t => (
            <button key={t.id} onClick={() => setTab(t.id)} style={{
              flex: 1, padding: '24px 0', background: 'transparent', border: 'none',
              borderBottom: tab === t.id ? '2px solid var(--ink)' : '2px solid transparent',
              marginBottom: -1, cursor: 'pointer', textAlign: 'left', display: 'flex', alignItems: 'center', gap: 14,
            }}>
              <span style={{ color: tab === t.id ? 'var(--ink)' : 'var(--ink-muted)', display: 'flex' }}>{t.icon}</span>
              <span style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
                <span style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 24, color: tab === t.id ? 'var(--ink)' : 'var(--ink-muted)', letterSpacing: '-0.035em' }}>{t.label}</span>
                <Mono>{t.sub}</Mono>
              </span>
            </button>
          ))}
        </div>

        <div style={{ padding: '40px 0' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 16, marginBottom: 24, flexWrap: 'wrap' }}>
            <button style={{ ...btnPrimary, padding: '14px 22px', fontSize: 14 }}>
              <Icon name="cloud-dl" size={15} color="var(--cream)" />
              {files[tab][0]}
              <span style={{ opacity: 0.55, fontFamily: 'var(--mono)', fontSize: 11 }}>{files[tab][1]}</span>
            </button>
            <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
              <Icon name="shield" size={14} color="var(--forest)" />
              <Mono>signed by Luca · SHA256 3f1b…a07c</Mono>
            </div>
          </div>
          <div style={{ background: 'var(--ink)', padding: 24, borderRadius: 'var(--r-2)', fontFamily: 'var(--mono)', fontSize: 13, color: 'var(--cream)', display: 'flex', flexDirection: 'column', gap: 8 }}>
            <div style={{ color: 'color-mix(in srgb, var(--cream) 55%, transparent)', fontSize: 10, letterSpacing: '0.06em', textTransform: 'uppercase', marginBottom: 4 }}>or install with</div>
            {installs[tab].map((line, i) => (
              <div key={i} style={{ display: 'flex', gap: 14, alignItems: 'center' }}>
                <span style={{ color: 'var(--clay-soft)' }}>$</span>
                <span style={{ flex: 1 }}>{line}</span>
                <span style={{ color: 'color-mix(in srgb, var(--cream) 45%, transparent)', fontSize: 11, cursor: 'pointer' }}>copy</span>
              </div>
            ))}
          </div>
        </div>

        {/* extras row */}
        <div style={{ marginTop: 24, padding: 24, background: 'var(--paper-2)', borderRadius: 'var(--r-3)', border: '1px solid var(--hairline)', display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 32 }}>
          {[
            ['Source', 'github.com/adagio/adagio'],
            ['Verify', 'minisign keys · https://thedarkpyotr.github.io/adagio/keys'],
            ['Mirror', 'EU · Frankfurt · 146 ms'],
          ].map(([h, b]) => (
            <div key={h}>
              <Mono>{h}</Mono>
              <div style={{ marginTop: 6, fontFamily: 'var(--body)', fontSize: 14, color: 'var(--ink)' }}>{b}</div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

/* tiny SVG platform glyphs */
function PenguinGlyph() {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <ellipse cx="12" cy="10" rx="5" ry="6"/>
      <circle cx="10" cy="9" r="0.8" fill="currentColor"/>
      <circle cx="14" cy="9" r="0.8" fill="currentColor"/>
      <path d="M10.5 12c.5.6 2.5.6 3 0"/>
      <path d="M8 16c-1 2-1 4 0 5M16 16c1 2 1 4 0 5"/>
      <path d="M9 19h6"/>
    </svg>
  );
}
function AppleGlyph() {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
      <path d="M16 13c0 4-3 7-4 7s-1.5-1-3-1-2 1-3 1-4-3-4-7c0-3 2-5 4-5 1 0 2 1 3 1s2-1 3-1c2 0 4 2 4 5Z"/>
      <path d="M13 5c1-1 2-2 3-2"/>
    </svg>
  );
}
function WindowsGlyph() {
  return (
    <svg width="22" height="22" viewBox="0 0 24 24" fill="none">
      <rect x="3" y="3"  width="8" height="8" fill="currentColor"/>
      <rect x="13" y="3" width="8" height="8" fill="currentColor"/>
      <rect x="3" y="13" width="8" height="8" fill="currentColor"/>
      <rect x="13" y="13" width="8" height="8" fill="currentColor"/>
    </svg>
  );
}

function WebFooter() {
  const cols = [
    ['Product',     ['Features', 'For Nextcloud', 'Roadmap', 'Changelog', 'Status']],
    ['Composition', ['Source', 'Security', 'Audit', 'Privacy', 'Telemetry']],
    ['Resources',   ['Docs', 'Self-host guide', 'Sync engine paper', 'Press kit', 'Brand']],
    ['Sound',       ['About', 'Manifesto', 'Contact', 'Mastodon', 'Bluesky']],
  ];
  return (
    <div style={{ padding: '96px 56px 32px', background: 'var(--cream)' }}>
      {/* giant wordmark — visual close */}
      <div style={{ paddingBottom: 64, borderBottom: '1px solid var(--hairline)' }}>
        <div style={{ display: 'flex', alignItems: 'flex-end', justifyContent: 'space-between', gap: 32 }}>
          <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 280, lineHeight: 0.85, letterSpacing: '-0.07em', color: 'var(--ink)' }}>
            adagio
          </div>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 8, alignItems: 'flex-end', paddingBottom: 24 }}>
            <Mono style={{ color: 'var(--ink)' }}>made in Munich</Mono>
            <Mono>by sound gmbh</Mono>
            <Mono>since 2024</Mono>
          </div>
        </div>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: '1.6fr repeat(4, 1fr)', gap: 48, padding: '56px 0', borderBottom: '1px solid var(--hairline)' }}>
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 12, marginBottom: 18 }}>
            <MarkSlur size={24} fill="var(--ink)" accent="var(--clay)" modern />
            <AdagioWord size={24} weight={500} />
          </div>
          <p style={{ fontSize: 14, lineHeight: 1.6, color: 'var(--ink-soft)', maxWidth: 280, margin: 0 }}>
            Adagio is made in Munich by a small studio called Sound. We design tools you stop noticing.
          </p>
          <div style={{ marginTop: 24, padding: '10px 14px', display: 'inline-flex', alignItems: 'center', gap: 10, background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)' }}>
            <span style={{ width: 7, height: 7, borderRadius: 4, background: 'var(--good)' }}/>
            <Mono style={{ color: 'var(--ink)' }}>All systems quiet</Mono>
          </div>
        </div>
        {cols.map(([title, items]) => (
          <div key={title}>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.08em', textTransform: 'uppercase', color: 'var(--ink-muted)', marginBottom: 18 }}>{title}</div>
            <div style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
              {items.map(it => (
                <a key={it} style={{ ...navLink, fontSize: 14, color: 'var(--ink)' }}>{it}</a>
              ))}
            </div>
          </div>
        ))}
      </div>
      <div style={{ marginTop: 28, display: 'flex', justifyContent: 'space-between', alignItems: 'center', fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)' }}>
        <span>© 2026 · Munich · AGPL-3.0</span>
        <span style={{ display: 'flex', gap: 24 }}>
          <span>Imprint</span>
          <span>Privacy</span>
          <span>RSS</span>
        </span>
      </div>
    </div>
  );
}

Object.assign(window, { Website });
