/* desktop-app.jsx — Adagio Linux client, interactive */

const { useState, useEffect, useMemo } = React;

/* ─── data ───────────────────────────────────────────── */

const ACCOUNTS = [
  { id: 'work', name: 'Work · Sound GmbH', host: 'cloud.sound.studio', initial: 'S', color: 'var(--forest)' },
  { id: 'home', name: 'Personal', host: 'nc.lena-h.de', initial: 'L', color: 'var(--clay)' },
];

const FILE_TREE = {
  '/Sound': [
    { name: 'Brand · Adagio',           kind: 'folder', size: '—',        mtime: 'today, 11:42',     status: 'sync',  share: 3, items: 84 },
    { name: 'Sync Engine — Whitepaper.pdf', kind: 'pdf',  size: '1.2 MB',  mtime: 'today, 09:18',     status: 'ok' },
    { name: 'Investor update Q2.md',    kind: 'md',     size: '14 KB',    mtime: 'yesterday',        status: 'ok',    share: 1 },
    { name: 'Engineering',              kind: 'folder', size: '—',        mtime: 'yesterday',        status: 'pin',   items: 312 },
    { name: 'Hiring · 2026',            kind: 'folder', size: '—',        mtime: '3 days ago',       status: 'ok',    items: 27 },
    { name: 'Composition.fig',          kind: 'fig',    size: '46.8 MB',  mtime: '3 days ago',       status: 'cloud' },
    { name: 'allegretto-press-kit.zip', kind: 'zip',    size: '128 MB',   mtime: 'last week',        status: 'cloud' },
    { name: 'Logo · construction.svg',  kind: 'svg',    size: '32 KB',    mtime: 'last week',        status: 'ok',    share: 2 },
    { name: 'Tempo — recording.wav',    kind: 'wav',    size: '212 MB',   mtime: 'last week',        status: 'cloud' },
    { name: 'Manifesto — draft.txt',    kind: 'txt',    size: '6 KB',     mtime: 'two weeks ago',    status: 'conflict' },
    { name: 'Old — archive',            kind: 'folder', size: '—',        mtime: 'a month ago',      status: 'ok',    items: 1241 },
  ],
};

const ACTIVITY = [
  { who: 'Mira Olsson',  what: 'edited', target: 'Investor update Q2.md',         where: '/Sound',  when: '4 min ago',  bullet: 'edit' },
  { who: 'You',          what: 'shared', target: 'Logo · construction.svg',       where: '/Brand', when: '12 min ago', bullet: 'share', to: 'Yui Tanaka' },
  { who: 'Auto-sync',    what: 'pulled', target: '7 files in Engineering/',       where: '/Sound',  when: '32 min ago', bullet: 'sync' },
  { who: 'Conflict',     what: 'flagged', target: 'Manifesto — draft.txt',        where: '/Sound', when: '1 hr ago',   bullet: 'conflict' },
  { who: 'You',          what: 'pinned', target: 'Engineering',                   where: '/Sound',  when: '2 hr ago',   bullet: 'pin' },
  { who: 'Felix Krüger', what: 'added',  target: 'Composition.fig',               where: '/Brand', when: '3 hr ago',  bullet: 'add' },
  { who: 'Mira Olsson',  what: 'edited', target: 'Adagio · style.css',             where: '/Brand', when: 'yesterday', bullet: 'edit' },
  { who: 'You',          what: 'declined', target: 'a share from external user',  where: '—',      when: 'yesterday',  bullet: 'block' },
  { who: 'Auto-sync',    what: 'pulled', target: '142 files',                     where: 'Hiring 2026', when: 'yesterday', bullet: 'sync' },
];

/* ─── icons ──────────────────────────────────────────── */

const Icon = ({ name, size = 16, color = 'currentColor', strokeWidth = 1.6 }) => {
  const s = size;
  const common = { width: s, height: s, viewBox: '0 0 24 24', fill: 'none', stroke: color, strokeWidth, strokeLinecap: 'round', strokeLinejoin: 'round', style: { display: 'block', flexShrink: 0 } };
  switch (name) {
    case 'folder':    return (<svg {...common}><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z"/></svg>);
    case 'file':      return (<svg {...common}><path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9l-6-6Z"/><path d="M14 3v6h6"/></svg>);
    case 'search':    return (<svg {...common}><circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/></svg>);
    case 'plus':      return (<svg {...common}><path d="M12 5v14M5 12h14"/></svg>);
    case 'chevron':   return (<svg {...common}><path d="m9 6 6 6-6 6"/></svg>);
    case 'caret-down':return (<svg {...common}><path d="m6 9 6 6 6-6"/></svg>);
    case 'check':     return (<svg {...common}><path d="M5 13l4 4L19 7"/></svg>);
    case 'cloud':     return (<svg {...common}><path d="M17 18a4 4 0 0 0 0-8 6 6 0 0 0-11.5 1.5A3.5 3.5 0 0 0 6.5 18H17Z"/></svg>);
    case 'cloud-dl':  return (<svg {...common}><path d="M17 18a4 4 0 0 0 0-8 6 6 0 0 0-11.5 1.5A3.5 3.5 0 0 0 6.5 18H17Z"/><path d="M12 12v6m0 0-2-2m2 2 2-2"/></svg>);
    case 'pin':       return (<svg {...common}><path d="M12 17v5"/><path d="M9 3h6l1 8h2v3H6v-3h2l1-8Z"/></svg>);
    case 'share':     return (<svg {...common}><circle cx="6" cy="12" r="2.5"/><circle cx="18" cy="6" r="2.5"/><circle cx="18" cy="18" r="2.5"/><path d="M8.5 11 16 7M8.5 13 16 17"/></svg>);
    case 'link':      return (<svg {...common}><path d="M10 13a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1"/><path d="M14 11a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1"/></svg>);
    case 'star':      return (<svg {...common}><path d="m12 3 2.6 5.5 6 .9-4.4 4.2 1 6.1L12 16.8 6.8 19.7l1-6.1L3.4 9.4l6-.9L12 3Z"/></svg>);
    case 'clock':     return (<svg {...common}><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>);
    case 'people':    return (<svg {...common}><circle cx="9" cy="9" r="3.5"/><path d="M3 19a6 6 0 0 1 12 0"/><circle cx="17" cy="9" r="2.5"/><path d="M16 19a5 5 0 0 1 5-5"/></svg>);
    case 'tag':       return (<svg {...common}><path d="M3 13V4h9l9 9-9 9-9-9Z"/><circle cx="8" cy="8" r="1.5" fill={color}/></svg>);
    case 'bell':      return (<svg {...common}><path d="M6 17V11a6 6 0 1 1 12 0v6l2 2H4l2-2Z"/><path d="M10 21a2 2 0 0 0 4 0"/></svg>);
    case 'settings':  return (<svg {...common}><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3h.1A1.7 1.7 0 0 0 10 3.1V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8v.1a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z"/></svg>);
    case 'menu':      return (<svg {...common}><path d="M5 7h14M5 12h14M5 17h14"/></svg>);
    case 'sync':      return (<svg {...common}><path d="M3 12a9 9 0 0 1 15-6.7L21 8"/><path d="M21 4v4h-4"/><path d="M21 12a9 9 0 0 1-15 6.7L3 16"/><path d="M3 20v-4h4"/></svg>);
    case 'check-circ':return (<svg {...common}><circle cx="12" cy="12" r="9"/><path d="m8 12 3 3 5-6"/></svg>);
    case 'warn':      return (<svg {...common}><path d="M12 3 2 21h20L12 3Z"/><path d="M12 10v5"/><circle cx="12" cy="18" r="0.8" fill={color}/></svg>);
    case 'globe':     return (<svg {...common}><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/></svg>);
    case 'min':       return (<svg {...common} viewBox="0 0 12 12"><path d="M2 6h8"/></svg>);
    case 'max':       return (<svg {...common} viewBox="0 0 12 12"><rect x="2" y="2" width="8" height="8"/></svg>);
    case 'close':     return (<svg {...common} viewBox="0 0 12 12"><path d="M2 2l8 8M10 2l-8 8"/></svg>);
    case 'x':         return (<svg {...common}><path d="M5 5l14 14M19 5 5 19"/></svg>);
    case 'eye':       return (<svg {...common}><path d="M2 12s4-7 10-7 10 7 10 7-4 7-10 7-10-7-10-7Z"/><circle cx="12" cy="12" r="3"/></svg>);
    case 'pencil':    return (<svg {...common}><path d="M4 20h4l10-10-4-4L4 16v4Z"/></svg>);
    case 'mail':      return (<svg {...common}><rect x="3" y="5" width="18" height="14" rx="2"/><path d="m3 7 9 6 9-6"/></svg>);
    case 'shield':    return (<svg {...common}><path d="M12 3 4 6v6c0 4.5 3.5 8 8 9 4.5-1 8-4.5 8-9V6l-8-3Z"/></svg>);
    case 'copy':      return (<svg {...common}><rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/></svg>);
    case 'arrow-r':   return (<svg {...common}><path d="M5 12h14m-6-6 6 6-6 6"/></svg>);
    default: return null;
  }
};

/* ─── file kind glyph (small letter chips) ─── */

const KIND_COLOR = {
  folder: 'var(--forest)', pdf: '#a8443a', md: 'var(--ink-soft)', fig: '#7a4b8a',
  zip: '#7d6b5b', svg: 'var(--clay)', wav: '#3a6e8a', txt: 'var(--ink-soft)',
};

function FileGlyph({ kind, status }) {
  if (kind === 'folder') {
    return (
      <div style={{ position: 'relative', width: 28, height: 22 }}>
        <svg width="28" height="22" viewBox="0 0 28 22" fill="none">
          <path d="M1 4a2 2 0 0 1 2-2h6l2 2h14a2 2 0 0 1 2 2v13a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V4Z" stroke="var(--forest)" strokeWidth="1.4" fill="var(--paper-2)"/>
        </svg>
      </div>
    );
  }
  return (
    <div style={{ position: 'relative', width: 26, height: 30, background: 'var(--paper-2)', border: '1px solid var(--hairline)', borderRadius: 3, display: 'flex', flexDirection: 'column', justifyContent: 'space-between', padding: '3px 0' }}>
      <div style={{ position: 'absolute', top: 0, right: 0, width: 8, height: 8, background: 'var(--cream-2)', borderLeft: '1px solid var(--hairline)', borderBottom: '1px solid var(--hairline)' }}/>
      <div style={{ flex: 1 }}/>
      <div style={{ fontFamily: 'var(--mono)', fontSize: 7, letterSpacing: '0.05em', textAlign: 'center', color: KIND_COLOR[kind] || 'var(--ink-soft)', fontWeight: 600, textTransform: 'uppercase' }}>{kind}</div>
    </div>
  );
}

/* ─── status badges ─── */

function StatusDot({ status, share }) {
  if (status === 'sync') return <SpinDot color="var(--clay)" />;
  if (status === 'ok') return <div title="In sync" style={{ width: 14, height: 14, borderRadius: 7, background: 'var(--good)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}><Icon name="check" size={9} color="white" strokeWidth={2.5}/></div>;
  if (status === 'cloud') return <Icon name="cloud" size={16} color="var(--ink-muted)"/>;
  if (status === 'pin') return <div title="Pinned offline" style={{ width: 14, height: 14, borderRadius: 7, background: 'var(--ink)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}><Icon name="check" size={9} color="var(--cream)" strokeWidth={2.5}/></div>;
  if (status === 'conflict') return <Icon name="warn" size={16} color="var(--danger)" strokeWidth={1.8}/>;
  return null;
}

function SpinDot({ color }) {
  return (
    <div style={{ width: 14, height: 14, position: 'relative' }} title="Syncing">
      <svg width="14" height="14" viewBox="0 0 14 14" style={{ animation: 'adagio-spin 1.4s linear infinite' }}>
        <circle cx="7" cy="7" r="5.5" stroke={color} strokeOpacity="0.25" strokeWidth="1.6" fill="none"/>
        <path d="M7 1.5a5.5 5.5 0 0 1 5.5 5.5" stroke={color} strokeWidth="1.6" strokeLinecap="round" fill="none"/>
      </svg>
      <style>{`@keyframes adagio-spin{to{transform:rotate(360deg)}}`}</style>
    </div>
  );
}

/* ─── window chrome (GNOME-ish) ─── */

function Chrome({ children, title = '', tab, onTab }) {
  return (
    <div style={{
      width: '100%', height: '100%', background: 'var(--cream)', display: 'flex', flexDirection: 'column',
      fontFamily: 'var(--body)', color: 'var(--ink)', borderRadius: 'inherit', overflow: 'hidden',
    }}>
      <div style={{
        height: 44, flexShrink: 0, display: 'flex', alignItems: 'center', padding: '0 14px 0 18px',
        borderBottom: '1px solid var(--hairline)', background: 'var(--paper)', gap: 14,
      }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <MarkSlur size={18} fill="var(--ink)" accent="var(--clay)" />
          <span style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 16, letterSpacing: '-0.05em' }}>adagio</span>
        </div>
        {tab && (
          <div style={{ display: 'flex', gap: 0, marginLeft: 18, padding: 3, background: 'var(--cream-2)', borderRadius: 'var(--r-pill)' }}>
            {[['files', 'Files'], ['activity', 'Activity']].map(([id, lbl]) => (
              <button key={id} onClick={() => onTab && onTab(id)}
                style={{
                  background: tab === id ? 'var(--cream)' : 'transparent',
                  color: tab === id ? 'var(--ink)' : 'var(--ink-muted)',
                  border: 'none', padding: '5px 14px', borderRadius: 'var(--r-pill)', fontSize: 12.5,
                  cursor: 'pointer', fontWeight: 500, boxShadow: tab === id ? '0 1px 2px rgba(0,0,0,0.06)' : 'none',
                }}>{lbl}</button>
            ))}
          </div>
        )}
        <div style={{ flex: 1, display: 'flex', justifyContent: 'center' }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 14px', background: 'var(--cream-2)',
            borderRadius: 'var(--r-pill)', width: 320, color: 'var(--ink-muted)', fontSize: 13 }}>
            <Icon name="search" size={14} color="var(--ink-muted)" />
            <span>Search files, people, activity…</span>
            <span style={{ marginLeft: 'auto', fontFamily: 'var(--mono)', fontSize: 10, background: 'var(--cream)', padding: '2px 5px', borderRadius: 3 }}>⌘K</span>
          </div>
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 6, color: 'var(--ink-soft)' }}>
          <ChromeBtn><Icon name="bell" size={15} /></ChromeBtn>
          <ChromeBtn><Icon name="settings" size={15} /></ChromeBtn>
          <div style={{ width: 26, height: 26, borderRadius: 13, background: 'var(--forest)', color: 'var(--cream)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontSize: 11, fontWeight: 600, marginLeft: 4 }}>S</div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 0, marginLeft: 10, paddingLeft: 10, borderLeft: '1px solid var(--hairline)' }}>
            <ChromeBtn><Icon name="min" size={11} strokeWidth={1.8} /></ChromeBtn>
            <ChromeBtn><Icon name="max" size={11} strokeWidth={1.8} /></ChromeBtn>
            <ChromeBtn><Icon name="close" size={11} strokeWidth={1.8} /></ChromeBtn>
          </div>
        </div>
      </div>
      {children}
    </div>
  );
}

function ChromeBtn({ children, onClick }) {
  const [h, setH] = useState(false);
  return (
    <button onClick={onClick}
      onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{
        width: 28, height: 28, background: h ? 'var(--cream-2)' : 'transparent',
        border: 'none', borderRadius: 6, cursor: 'pointer', color: 'var(--ink-soft)',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
      }}>{children}</button>
  );
}

/* ─── sidebar ─── */

function Sidebar({ selected, onSelect, account, onAccount }) {
  return (
    <div style={{ width: 248, flexShrink: 0, background: 'var(--paper)', borderRight: '1px solid var(--hairline)',
      display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
      {/* account picker */}
      <div style={{ padding: '14px 14px 12px' }}>
        <button onClick={() => onAccount && onAccount()} style={{ width: '100%', background: 'var(--cream-2)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', padding: 10, display: 'flex', alignItems: 'center', gap: 10, cursor: 'pointer', textAlign: 'left' }}>
          <div style={{ width: 30, height: 30, borderRadius: 8, background: account.color, color: 'var(--cream)', display: 'flex', alignItems: 'center', justifyContent: 'center', fontFamily: 'var(--body)', fontWeight: 600, fontSize: 14, letterSpacing: '-0.04em' }}>{account.initial}</div>
          <div style={{ flex: 1, minWidth: 0 }}>
            <div style={{ fontSize: 13, fontWeight: 500, color: 'var(--ink)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{account.name}</div>
            <div style={{ fontSize: 11, color: 'var(--ink-muted)', fontFamily: 'var(--mono)', letterSpacing: '0.02em', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{account.host}</div>
          </div>
          <Icon name="caret-down" size={13} color="var(--ink-muted)" />
        </button>
      </div>

      <SidebarSection label="Library">
        <SidebarItem icon="folder" label="All files" badge="3.2k" id="all" selected={selected} onSelect={onSelect} />
        <SidebarItem icon="star" label="Favorites" badge="14" id="fav" selected={selected} onSelect={onSelect} />
        <SidebarItem icon="clock" label="Recent" id="recent" selected={selected} onSelect={onSelect} />
        <SidebarItem icon="people" label="Shared" badge="8" id="shared" selected={selected} onSelect={onSelect} />
        <SidebarItem icon="tag" label="Tagged" id="tags" selected={selected} onSelect={onSelect} />
      </SidebarSection>

      <SidebarSection label="Pinned folders">
        <PinnedFolder name="Brand · Adagio" status="ok" />
        <PinnedFolder name="Engineering" status="ok" />
        <PinnedFolder name="Hiring · 2026" status="sync" />
      </SidebarSection>

      <div style={{ flex: 1 }}/>

      <SyncFooter />
    </div>
  );
}

function SidebarSection({ label, children }) {
  return (
    <div style={{ padding: '4px 10px 8px' }}>
      <div style={{ padding: '6px 8px 8px', fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.16em', textTransform: 'uppercase', color: 'var(--ink-muted)' }}>{label}</div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 1 }}>{children}</div>
    </div>
  );
}

function SidebarItem({ icon, label, badge, id, selected, onSelect }) {
  const active = selected === id;
  const [h, setH] = useState(false);
  return (
    <button onClick={() => onSelect && onSelect(id)}
      onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{
        background: active ? 'var(--cream-2)' : h ? 'color-mix(in srgb, var(--ink) 5%, transparent)' : 'transparent',
        border: 'none', borderRadius: 'var(--r-2)', padding: '7px 9px', display: 'flex', alignItems: 'center',
        gap: 10, fontSize: 13, color: active ? 'var(--ink)' : 'var(--ink-soft)', cursor: 'pointer',
        fontWeight: active ? 500 : 400, textAlign: 'left',
      }}>
      <Icon name={icon} size={15} color={active ? 'var(--clay)' : 'var(--ink-muted)'} />
      <span style={{ flex: 1 }}>{label}</span>
      {badge && <span style={{ fontSize: 10.5, color: 'var(--ink-muted)', fontFamily: 'var(--mono)' }}>{badge}</span>}
    </button>
  );
}

function PinnedFolder({ name, status }) {
  return (
    <div style={{ padding: '6px 9px', display: 'flex', alignItems: 'center', gap: 10, fontSize: 13, color: 'var(--ink-soft)' }}>
      <div style={{ width: 4, height: 4, borderRadius: 2, background: status === 'sync' ? 'var(--clay)' : 'var(--good)' }}/>
      <span style={{ flex: 1, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>{name}</span>
      {status === 'sync' && <SpinDot color="var(--clay)" />}
    </div>
  );
}

function SyncFooter() {
  return (
    <div style={{ padding: '12px 14px', borderTop: '1px solid var(--hairline)', display: 'flex', flexDirection: 'column', gap: 8 }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <SpinDot color="var(--clay)" />
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: 12, color: 'var(--ink)', fontWeight: 500 }}>Syncing 7 files</div>
          <div style={{ fontSize: 11, color: 'var(--ink-muted)', fontFamily: 'var(--mono)', whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>~ 2.1 MB / 14.6 MB · 38s</div>
        </div>
      </div>
      <div style={{ height: 3, background: 'var(--cream-2)', borderRadius: 2, overflow: 'hidden' }}>
        <div style={{ width: '38%', height: '100%', background: 'var(--clay)' }}/>
      </div>
    </div>
  );
}

Object.assign(window, { Icon, MarkSlur: window.MarkSlur, FileGlyph, StatusDot, SpinDot, Chrome, Sidebar, ACCOUNTS, FILE_TREE, ACTIVITY });
