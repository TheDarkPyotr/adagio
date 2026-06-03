/* logos.jsx — three Adagio logo directions + a construction board (modern) */

function LogoFermata() {
  return (
    <div style={{
      width: '100%', height: '100%', background: 'var(--cream)',
      display: 'flex', flexDirection: 'column', justifyContent: 'space-between',
      padding: '28px 32px', position: 'relative',
    }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span className="eyebrow">01 / wordmark</span>
        <span className="eyebrow">fermata</span>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 16 }}>
        <svg width="36" height="18" viewBox="0 0 36 18" fill="none" style={{ color: 'var(--clay)' }}>
          <path d="M2 15 C 2 4, 34 4, 34 15" stroke="currentColor" strokeWidth="1.2" strokeLinecap="round" fill="none"/>
          <circle cx="18" cy="12" r="1.7" fill="currentColor"/>
        </svg>
        <div style={{ fontFamily: 'var(--body)', fontWeight: 400, fontSize: 124, lineHeight: 1, color: 'var(--ink)', letterSpacing: '-0.055em' }}>
          adagio
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10, marginTop: 4 }}>
          <span style={{ width: 18, height: 1, background: 'var(--ink-muted)' }}/>
          <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontSize: 18, color: 'var(--ink-soft)', letterSpacing: '-0.01em' }}>files, at a graceful tempo</span>
          <span style={{ width: 18, height: 1, background: 'var(--ink-muted)' }}/>
        </div>
      </div>

      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end' }}>
        <div style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--ink-muted)', lineHeight: 1.6 }}>
          Geist · 400 · 124 / 100<br/>tracking −5.5%
        </div>
        <div style={{ display: 'flex', gap: 4, alignItems: 'center' }}>
          <Swatch color="var(--ink)" />
          <Swatch color="var(--clay)" />
          <Swatch color="var(--cream)" border />
        </div>
      </div>
    </div>
  );
}

function LogoSlur() {
  return (
    <div style={{ width: '100%', height: '100%', background: 'var(--ink)', color: 'var(--cream)',
      display: 'flex', flexDirection: 'column', justifyContent: 'space-between',
      padding: '28px 32px', position: 'relative',
    }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span className="eyebrow" style={{ color: 'rgba(245,241,234,0.55)' }}>02 / mark + wordmark</span>
        <span className="eyebrow" style={{ color: 'rgba(245,241,234,0.55)' }}>system primary</span>
      </div>

      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'center', gap: 22 }}>
        <MarkSlur size={100} fill="var(--cream)" accent="var(--clay)" strokeOpacity={1} modern />
        <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
          <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 96, lineHeight: 0.95, letterSpacing: '-0.06em' }}>
            adagio
          </div>
          <div style={{ fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '0.18em', textTransform: 'uppercase', color: 'rgba(245,241,234,0.5)', marginLeft: 2 }}>
            for nextcloud
          </div>
        </div>
      </div>

      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end' }}>
        <div style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'rgba(245,241,234,0.55)', lineHeight: 1.6 }}>
          mark · 1:1 grid<br/>monoline · 6% stroke
        </div>
        <div style={{ display: 'flex', gap: 4 }}>
          <Swatch color="var(--cream)" />
          <Swatch color="var(--clay)" />
          <Swatch color="var(--ink)" border />
        </div>
      </div>
    </div>
  );
}

function LogoMetronome() {
  return (
    <div style={{ width: '100%', height: '100%', background: 'var(--paper)',
      display: 'flex', flexDirection: 'column', justifyContent: 'space-between',
      padding: '28px 32px', position: 'relative',
    }}>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <span className="eyebrow">03 / monogram</span>
        <span className="eyebrow">app icon · square</span>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 24 }}>
        {/* "a" with a slur — the chosen monogram */}
        <div style={{ width: 152, height: 152, borderRadius: 32, background: 'var(--ink)', display: 'flex', alignItems: 'center', justifyContent: 'center', position: 'relative', boxShadow: '0 16px 40px rgba(21,23,26,0.18)' }}>
          {/* subtle slur arc above the a */}
          <svg width="92" height="22" viewBox="0 0 92 22" style={{ position: 'absolute', top: 26 }} fill="none">
            <path d="M6 18 C 18 4, 74 4, 86 18" stroke="var(--clay)" strokeWidth="2" strokeLinecap="round" fill="none"/>
          </svg>
          <span style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 108, color: 'var(--cream)', letterSpacing: '-0.08em', lineHeight: 1, marginTop: 22 }}>a</span>
        </div>
        <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 36, lineHeight: 1, color: 'var(--ink)', letterSpacing: '-0.05em' }}>
          adagio
        </div>
      </div>

      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end' }}>
        <div style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--ink-muted)', lineHeight: 1.6 }}>
          tile · 152 · radius 21%<br/>'a' optical center
        </div>
        <div style={{ display: 'flex', gap: 4 }}>
          <Swatch color="var(--ink)" />
          <Swatch color="var(--clay)" />
          <Swatch color="var(--paper)" border />
        </div>
      </div>
    </div>
  );
}

function LogoSystem() {
  return (
    <div style={{ width: '100%', height: '100%', background: 'var(--cream)',
      display: 'grid', gridTemplateColumns: '1.3fr 1fr 1fr', gap: 1, padding: 1,
    }}>
      {/* primary lock-up */}
      <div style={{ background: 'var(--cream)', padding: '26px 32px', display: 'flex', flexDirection: 'column', justifyContent: 'space-between' }}>
        <span className="eyebrow">Primary lock-up</span>
        <div style={{ display: 'flex', alignItems: 'center', gap: 18 }}>
          <MarkSlur size={48} fill="var(--ink)" accent="var(--clay)" modern />
          <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 60, lineHeight: 1, letterSpacing: '-0.055em' }}>
            adagio
          </div>
        </div>
        <span style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--ink-muted)' }}>
          clearspace · x = cap-height
        </span>
      </div>

      {/* scale */}
      <div style={{ background: 'var(--paper-2)', padding: '26px 28px', display: 'flex', flexDirection: 'column', justifyContent: 'space-between' }}>
        <span className="eyebrow">Scale</span>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 12, alignItems: 'flex-start' }}>
          <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 40, lineHeight: 1, letterSpacing: '-0.05em' }}>adagio</div>
          <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 24, lineHeight: 1, letterSpacing: '-0.05em' }}>adagio</div>
          <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 16, lineHeight: 1, letterSpacing: '-0.04em' }}>adagio</div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
            <MarkSlur size={12} fill="var(--ink)" accent="var(--clay)" modern />
            <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 11, lineHeight: 1, letterSpacing: '-0.03em' }}>adagio</div>
          </div>
        </div>
        <span style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'var(--ink-muted)' }}>
          min · 11px screen
        </span>
      </div>

      {/* app icon */}
      <div style={{ background: 'var(--ink)', padding: '26px 28px', color: 'var(--cream)', display: 'flex', flexDirection: 'column', justifyContent: 'space-between' }}>
        <span className="eyebrow" style={{ color: 'rgba(245,241,234,0.55)' }}>App icon</span>
        <div style={{ display: 'flex', alignItems: 'flex-end', gap: 12 }}>
          <AppTile size={88} />
          <AppTile size={46} />
          <AppTile size={24} />
        </div>
        <span style={{ fontFamily: 'var(--mono)', fontSize: 10, color: 'rgba(245,241,234,0.55)' }}>
          radius 21% · light tile
        </span>
      </div>
    </div>
  );
}

function AppTile({ size }) {
  const r = Math.round(size * 0.21);
  const arcTop = size * 0.22;
  return (
    <div style={{
      width: size, height: size, borderRadius: r, background: 'var(--cream)',
      display: 'flex', alignItems: 'center', justifyContent: 'center', position: 'relative',
    }}>
      {size >= 40 && (
        <svg width={size * 0.6} height={size * 0.16} viewBox="0 0 92 22" style={{ position: 'absolute', top: arcTop }} fill="none">
          <path d="M6 18 C 18 4, 74 4, 86 18" stroke="var(--clay)" strokeWidth={size >= 70 ? 2 : 2.4} strokeLinecap="round" fill="none"/>
        </svg>
      )}
      <span style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: size * 0.7, color: 'var(--ink)', letterSpacing: '-0.08em', lineHeight: 1, marginTop: size >= 40 ? size * 0.14 : 0 }}>a</span>
    </div>
  );
}

/* The slur mark — modern monoline version */
function MarkSlur({ size = 24, fill = 'currentColor', accent = '#c8542a', strokeOpacity = 1, modern = true }) {
  // 60×60 viewBox kept for compat; monoline stroke + small note circles
  const s = size;
  if (modern) {
    return (
      <svg width={s} height={s} viewBox="0 0 60 60" fill="none" style={{ display: 'block' }}>
        {/* the slur arc */}
        <path d="M8 42 C 18 14, 42 14, 52 42" stroke={fill} strokeWidth="3" strokeLinecap="round" fill="none" opacity={strokeOpacity}/>
        {/* two notes — solid, smaller */}
        <circle cx="8" cy="42" r="5" fill={fill}/>
        <circle cx="52" cy="42" r="5" fill={accent}/>
      </svg>
    );
  }
  // fallback: original ornamental version
  return (
    <svg width={s} height={s} viewBox="0 0 60 60" fill="none" style={{ display: 'block' }}>
      <circle cx="11" cy="42" r="7" fill={fill}/>
      <circle cx="49" cy="42" r="7" fill={accent}/>
      <path d="M7 32 C 16 7, 44 7, 53 32" stroke={fill} strokeWidth="2" strokeLinecap="round" fill="none" opacity={strokeOpacity}/>
      <line x1="17.5" y1="42" x2="17.5" y2="14" stroke={fill} strokeWidth="1.6" strokeLinecap="round" opacity="0.55"/>
      <line x1="55.5" y1="42" x2="55.5" y2="14" stroke={accent} strokeWidth="1.6" strokeLinecap="round" opacity="0.7"/>
    </svg>
  );
}

function Swatch({ color, border }) {
  return <div style={{ width: 12, height: 12, borderRadius: 2, background: color, boxShadow: border ? 'inset 0 0 0 1px var(--hairline)' : 'none' }}/>;
}

/* Wordmark only — reusable, modern sans */
function AdagioWord({ size = 32, color = 'var(--ink)', weight = 500 }) {
  return (
    <span style={{ fontFamily: 'var(--body)', fontWeight: weight, fontSize: size, lineHeight: 1, letterSpacing: '-0.05em', color }}>
      adagio
    </span>
  );
}

Object.assign(window, { LogoFermata, LogoSlur, LogoMetronome, LogoSystem, MarkSlur, AdagioWord });
