import React, { useState } from 'react';

// ── MarkSlur logo ──────────────────────────────────────────────────────────────

export function MarkSlur({ size = 24, fill = 'currentColor', accent = '#c8542a' }: {
  size?: number; fill?: string; accent?: string;
}) {
  return (
    <svg width={size} height={size} viewBox="0 0 60 60" fill="none" style={{ display: 'block' }}>
      <path d="M8 42 C 18 14, 42 14, 52 42" stroke={fill} strokeWidth="3" strokeLinecap="round" fill="none"/>
      <circle cx="8" cy="42" r="5" fill={fill}/>
      <circle cx="52" cy="42" r="5" fill={accent}/>
    </svg>
  );
}

// ── Icon ──────────────────────────────────────────────────────────────────────

export type IconName =
  | 'folder' | 'file' | 'search' | 'plus' | 'chevron' | 'caret-down' | 'check'
  | 'cloud' | 'cloud-dl' | 'pin' | 'share' | 'link' | 'star' | 'clock' | 'people'
  | 'tag' | 'bell' | 'settings' | 'menu' | 'sync' | 'check-circ' | 'warn' | 'globe'
  | 'min' | 'max' | 'close' | 'x' | 'eye' | 'pencil' | 'mail' | 'shield' | 'copy'
  | 'arrow-r' | 'upload' | 'file-plus' | 'folder-plus' | 'trash' | 'refresh';

export function Icon({ name, size = 16, color = 'currentColor', strokeWidth = 1.6 }: {
  name: IconName; size?: number; color?: string; strokeWidth?: number;
}) {
  const s = size;
  const common: React.SVGProps<SVGSVGElement> = {
    width: s, height: s, viewBox: '0 0 24 24', fill: 'none',
    stroke: color, strokeWidth, strokeLinecap: 'round', strokeLinejoin: 'round',
    style: { display: 'block', flexShrink: 0 },
  };
  switch (name) {
    case 'folder':    return <svg {...common}><path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V7Z"/></svg>;
    case 'file':      return <svg {...common}><path d="M14 3H6a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9l-6-6Z"/><path d="M14 3v6h6"/></svg>;
    case 'search':    return <svg {...common}><circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/></svg>;
    case 'plus':      return <svg {...common}><path d="M12 5v14M5 12h14"/></svg>;
    case 'chevron':   return <svg {...common}><path d="m9 6 6 6-6 6"/></svg>;
    case 'caret-down':return <svg {...common}><path d="m6 9 6 6 6-6"/></svg>;
    case 'check':     return <svg {...common}><path d="M5 13l4 4L19 7"/></svg>;
    case 'cloud':     return <svg {...common}><path d="M17 18a4 4 0 0 0 0-8 6 6 0 0 0-11.5 1.5A3.5 3.5 0 0 0 6.5 18H17Z"/></svg>;
    case 'cloud-dl':  return <svg {...common}><path d="M17 18a4 4 0 0 0 0-8 6 6 0 0 0-11.5 1.5A3.5 3.5 0 0 0 6.5 18H17Z"/><path d="M12 12v6m0 0-2-2m2 2 2-2"/></svg>;
    case 'pin':       return <svg {...common}><path d="M12 17v5"/><path d="M9 3h6l1 8h2v3H6v-3h2l1-8Z"/></svg>;
    case 'share':     return <svg {...common}><circle cx="6" cy="12" r="2.5"/><circle cx="18" cy="6" r="2.5"/><circle cx="18" cy="18" r="2.5"/><path d="M8.5 11 16 7M8.5 13 16 17"/></svg>;
    case 'link':      return <svg {...common}><path d="M10 13a5 5 0 0 0 7 0l3-3a5 5 0 0 0-7-7l-1 1"/><path d="M14 11a5 5 0 0 0-7 0l-3 3a5 5 0 0 0 7 7l1-1"/></svg>;
    case 'star':      return <svg {...common}><path d="m12 3 2.6 5.5 6 .9-4.4 4.2 1 6.1L12 16.8 6.8 19.7l1-6.1L3.4 9.4l6-.9L12 3Z"/></svg>;
    case 'clock':     return <svg {...common}><circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/></svg>;
    case 'people':    return <svg {...common}><circle cx="9" cy="9" r="3.5"/><path d="M3 19a6 6 0 0 1 12 0"/><circle cx="17" cy="9" r="2.5"/><path d="M16 19a5 5 0 0 1 5-5"/></svg>;
    case 'tag':       return <svg {...common}><path d="M3 13V4h9l9 9-9 9-9-9Z"/><circle cx="8" cy="8" r="1.5" fill={color}/></svg>;
    case 'bell':      return <svg {...common}><path d="M6 17V11a6 6 0 1 1 12 0v6l2 2H4l2-2Z"/><path d="M10 21a2 2 0 0 0 4 0"/></svg>;
    case 'settings':  return <svg {...common}><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3h.1A1.7 1.7 0 0 0 10 3.1V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8v.1a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1Z"/></svg>;
    case 'sync':      return <svg {...common}><path d="M3 12a9 9 0 0 1 15-6.7L21 8"/><path d="M21 4v4h-4"/><path d="M21 12a9 9 0 0 1-15 6.7L3 16"/><path d="M3 20v-4h4"/></svg>;
    case 'check-circ':return <svg {...common}><circle cx="12" cy="12" r="9"/><path d="m8 12 3 3 5-6"/></svg>;
    case 'warn':      return <svg {...common}><path d="M12 3 2 21h20L12 3Z"/><path d="M12 10v5"/><circle cx="12" cy="18" r="0.8" fill={color}/></svg>;
    case 'globe':     return <svg {...common}><circle cx="12" cy="12" r="9"/><path d="M3 12h18M12 3a14 14 0 0 1 0 18M12 3a14 14 0 0 0 0 18"/></svg>;
    case 'min':       return <svg {...common} viewBox="0 0 12 12"><path d="M2 6h8"/></svg>;
    case 'max':       return <svg {...common} viewBox="0 0 12 12"><rect x="2" y="2" width="8" height="8"/></svg>;
    case 'close':     return <svg {...common} viewBox="0 0 12 12"><path d="M2 2l8 8M10 2l-8 8"/></svg>;
    case 'x':         return <svg {...common}><path d="M5 5l14 14M19 5 5 19"/></svg>;
    case 'pencil':    return <svg {...common}><path d="M4 20h4l10-10-4-4L4 16v4Z"/></svg>;
    case 'shield':    return <svg {...common}><path d="M12 3 4 6v6c0 4.5 3.5 8 8 9 4.5-1 8-4.5 8-9V6l-8-3Z"/></svg>;
    case 'copy':      return <svg {...common}><rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/></svg>;
    case 'arrow-r':    return <svg {...common}><path d="M5 12h14m-6-6 6 6-6 6"/></svg>;
    case 'upload':     return <svg {...common}><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="17 8 12 3 7 8"/><line x1="12" y1="3" x2="12" y2="15"/></svg>;
    case 'file-plus':  return <svg {...common}><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/><line x1="12" y1="18" x2="12" y2="12"/><line x1="9" y1="15" x2="15" y2="15"/></svg>;
    case 'folder-plus': return <svg {...common}><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 2h9a2 2 0 0 1 2 2v11z"/><line x1="12" y1="11" x2="12" y2="17"/><line x1="9" y1="14" x2="15" y2="14"/></svg>;
    case 'trash':       return <svg {...common}><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14H6L5 6"/><path d="M10 11v6M14 11v6"/><path d="M9 6V4h6v2"/></svg>;
    case 'refresh':     return <svg {...common}><path d="M3 12a9 9 0 0 1 15-6.7L21 8"/><path d="M21 4v4h-4"/><path d="M21 12a9 9 0 0 1-15 6.7L3 16"/><path d="M3 20v-4h4"/></svg>;
    default:           return null;
  }
}

// ── SpinDot ────────────────────────────────────────────────────────────────────

export function SpinDot({ color = 'var(--clay)', size = 14 }: { color?: string; size?: number }) {
  return (
    <div style={{ width: size, height: size, position: 'relative', flexShrink: 0 }} title="Syncing">
      <svg width={size} height={size} viewBox="0 0 14 14" style={{ animation: 'adagio-spin 1.4s linear infinite', display: 'block' }}>
        <circle cx="7" cy="7" r="5.5" stroke={color} strokeOpacity="0.25" strokeWidth="1.6" fill="none"/>
        <path d="M7 1.5a5.5 5.5 0 0 1 5.5 5.5" stroke={color} strokeWidth="1.6" strokeLinecap="round" fill="none"/>
      </svg>
    </div>
  );
}

// ── FileGlyph ──────────────────────────────────────────────────────────────────

const KIND_COLOR: Record<string, string> = {
  folder: 'var(--forest)', pdf: '#a8443a', md: 'var(--ink-soft)', fig: '#7a4b8a',
  zip: '#7d6b5b', svg: 'var(--clay)', wav: '#3a6e8a', txt: 'var(--ink-soft)',
};

export function FileGlyph({ kind }: { kind: string }) {
  if (kind === 'folder') {
    return (
      <div style={{ position: 'relative', width: 28, height: 22 }}>
        <svg width="28" height="22" viewBox="0 0 28 22" fill="none">
          <path d="M1 4a2 2 0 0 1 2-2h6l2 2h14a2 2 0 0 1 2 2v13a2 2 0 0 1-2 2H3a2 2 0 0 1-2-2V4Z"
            stroke="var(--forest)" strokeWidth="1.4" fill="var(--paper-2)"/>
        </svg>
      </div>
    );
  }
  return (
    <div style={{ position: 'relative', width: 26, height: 30, background: 'var(--paper-2)', border: '1px solid var(--hairline)', borderRadius: 3, display: 'flex', flexDirection: 'column', justifyContent: 'space-between', padding: '3px 0' }}>
      <div style={{ position: 'absolute', top: 0, right: 0, width: 8, height: 8, background: 'var(--cream-2)', borderLeft: '1px solid var(--hairline)', borderBottom: '1px solid var(--hairline)' }}/>
      <div style={{ flex: 1 }}/>
      <div style={{ fontFamily: 'var(--mono)', fontSize: 7, letterSpacing: '0.05em', textAlign: 'center', color: KIND_COLOR[kind] ?? 'var(--ink-soft)', fontWeight: 600, textTransform: 'uppercase' }}>{kind}</div>
    </div>
  );
}

// ── StatusDot ──────────────────────────────────────────────────────────────────

export function StatusDot({ status }: { status: string }) {
  if (status === 'sync') return <SpinDot color="var(--clay)" />;
  if (status === 'ok') return (
    <div title="In sync" style={{ width: 14, height: 14, borderRadius: 7, background: 'var(--good)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
      <Icon name="check" size={9} color="white" strokeWidth={2.5}/>
    </div>
  );
  if (status === 'cloud') return <Icon name="cloud" size={16} color="var(--ink-muted)"/>;
  if (status === 'pin') return (
    <div title="Pinned offline" style={{ width: 14, height: 14, borderRadius: 7, background: 'var(--ink)', display: 'flex', alignItems: 'center', justifyContent: 'center', flexShrink: 0 }}>
      <Icon name="check" size={9} color="var(--cream)" strokeWidth={2.5}/>
    </div>
  );
  if (status === 'conflict') return <Icon name="warn" size={16} color="var(--danger)" strokeWidth={1.8}/>;
  return null;
}

// ── ChromeBtn ──────────────────────────────────────────────────────────────────

export function ChromeBtn({ children, onClick }: { children: React.ReactNode; onClick?: () => void }) {
  const [h, setH] = useState(false);
  return (
    <button onClick={onClick}
      onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{ width: 28, height: 28, background: h ? 'var(--cream-2)' : 'transparent', border: 'none', borderRadius: 6, cursor: 'pointer', color: 'var(--ink-soft)', display: 'flex', alignItems: 'center', justifyContent: 'center' }}>
      {children}
    </button>
  );
}

// ── FlatBtn ────────────────────────────────────────────────────────────────────

export function FlatBtn({ icon, label, primary, onClick, disabled }: {
  icon: IconName; label: string; primary?: boolean; onClick?: () => void; disabled?: boolean;
}) {
  const [h, setH] = useState(false);
  const style: React.CSSProperties = primary
    ? { background: h && !disabled ? 'var(--ink-2)' : 'var(--ink)', color: 'var(--cream)', border: 'none', opacity: disabled ? 0.4 : 1, cursor: disabled ? 'not-allowed' : 'pointer' }
    : { background: h ? 'var(--paper-2)' : 'transparent', color: 'var(--ink-soft)', border: '1px solid var(--hairline)', cursor: 'pointer' };
  return (
    <button
      onClick={!disabled ? onClick : undefined}
      onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{ ...style, padding: '7px 12px', borderRadius: 'var(--r-2)', display: 'flex', alignItems: 'center', gap: 7, fontSize: 12.5, fontWeight: 500 }}
    >
      <Icon name={icon} size={14} color={primary ? 'var(--cream)' : 'var(--ink-soft)'} />
      <span>{label}</span>
    </button>
  );
}
