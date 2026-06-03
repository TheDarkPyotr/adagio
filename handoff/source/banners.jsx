/* banners.jsx — GitHub README banner options */

/* ─── 1 · Composition (editorial — wordmark centered, mark above) ─── */
function BannerComposition() {
  return (
    <div style={{
      width: '100%', height: '100%', background: 'var(--cream)', color: 'var(--ink)',
      display: 'flex', flexDirection: 'column', position: 'relative', overflow: 'hidden',
      padding: '32px 56px', fontFamily: 'var(--body)'
    }}>
      {/* top row */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
          <span style={{ width: 7, height: 7, borderRadius: 1, background: 'var(--clay)' }} />
          <span style={{ fontFamily: 'var(--mono)', fontSize: 13, letterSpacing: '0.06em', textTransform: 'uppercase', color: 'var(--ink-muted)' }}>
            a desktop client for nextcloud
          </span>
        </div>
        <div style={{ fontFamily: 'var(--mono)', fontSize: 13, color: 'var(--ink-muted)', letterSpacing: '0.04em' }}>
          github.com / adagio / adagio
        </div>
      </div>

      {/* center: mark + wordmark + accent */}
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 28 }}>
        <MarkSlur size={88} fill="var(--ink)" accent="var(--clay)" modern />
        <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 184, lineHeight: 1, letterSpacing: '-0.06em' }}>
          adagio
        </div>
        <div style={{ display: 'flex', alignItems: 'center', gap: 16 }}>
          <span style={{ width: 32, height: 1, background: 'var(--ink-muted)' }} />
          <span style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontSize: 28, color: 'var(--ink-soft)', letterSpacing: '-0.01em' }}>
            the unhurried way to keep files in sync
          </span>
          <span style={{ width: 32, height: 1, background: 'var(--ink-muted)' }} />
        </div>
      </div>

      {/* bottom row */}
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end' }}>
        <span style={{ fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--ink-muted)' }}>
          v1.0 · allegretto
        </span>
        <span style={{ fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--ink-muted)' }}>
          AGPL-3.0 · linux · macOS · windows
        </span>
      </div>
    </div>);

}

/* ─── 2 · Gesture (oversized mark on left, content on right) ─── */
function BannerGesture() {
  return (
    <div style={{
      width: '100%', height: '100%', background: 'var(--ink)', color: 'var(--cream)',
      display: 'grid', gridTemplateColumns: '0.9fr 1.4fr',
      position: 'relative', overflow: 'hidden', fontFamily: 'var(--body)'
    }}>
      {/* left: oversized slur mark */}
      <div style={{ position: 'relative', overflow: 'hidden', borderRight: '1px solid color-mix(in srgb, var(--cream) 12%, transparent)' }}>
        <svg width="100%" height="100%" viewBox="0 0 460 480" preserveAspectRatio="xMidYMid meet" style={{ position: 'absolute', inset: 0 }} fill="none">
          {/* the slur gesture, much larger */}
          <path d="M60 360 C 110 80, 350 80, 400 360" stroke="var(--cream)" strokeWidth="14" strokeLinecap="round" fill="none" />
          <circle cx="60" cy="360" r="38" fill="var(--cream)" />
          <circle cx="400" cy="360" r="38" fill="var(--clay)" />
          {/* construction marks */}
          <circle cx="230" cy="80" r="3" fill="color-mix(in srgb, var(--cream) 35%, transparent)" />
          <circle cx="230" cy="430" r="3" fill="color-mix(in srgb, var(--cream) 35%, transparent)" />
        </svg>
        <span style={{ position: 'absolute', top: 24, left: 32, fontFamily: 'var(--mono)', fontSize: 11, letterSpacing: '0.06em', color: 'color-mix(in srgb, var(--cream) 55%, transparent)' }}>
          fig 01 · mark
        </span>
      </div>

      {/* right: copy */}
      <div style={{ padding: '40px 56px', display: 'flex', flexDirection: 'column', justifyContent: 'space-between' }}>
        <div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 10 }}>
            <span style={{ width: 7, height: 7, borderRadius: 1, background: 'var(--clay)' }} />
            <span style={{ fontFamily: 'var(--mono)', fontSize: 13, letterSpacing: '0.06em', textTransform: 'uppercase', color: 'color-mix(in srgb, var(--cream) 60%, transparent)' }}>
              a desktop client for nextcloud
            </span>
          </div>
        </div>

        <div style={{ display: 'flex', flexDirection: 'column', gap: 24 }}>
          <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 142, lineHeight: 0.95, letterSpacing: '-0.06em' }}>
            adagio
          </div>
          <div style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontSize: 32, lineHeight: 1.15, letterSpacing: '-0.01em', color: 'color-mix(in srgb, var(--cream) 80%, transparent)', maxWidth: 600 }}>
            files at a graceful tempo —<br />
            quiet, deliberate, out of your way.
          </div>
        </div>

        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'flex-end' }}>
          <div style={{ display: 'flex', gap: 22, fontFamily: 'var(--mono)', fontSize: 12, color: 'color-mix(in srgb, var(--cream) 60%, transparent)' }}>
            <span>v1.0 · allegretto</span>
            <span style={{ opacity: 0.5 }}>·</span>
            <span>AGPL-3.0</span>
            <span style={{ opacity: 0.5 }}>·</span>
            <span>4.2 MB</span>
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <span style={{
              padding: '6px 12px', background: 'var(--cream)', color: 'var(--ink)',
              borderRadius: 'var(--r-2)', fontSize: 12, fontWeight: 500,
              fontFamily: 'var(--mono)'
            }}>$ apt install adagio</span>
          </div>
        </div>
      </div>
    </div>);

}

/* ─── 3 · Score (sheet-music inspired with staff lines) ─── */
function BannerScore() {
  return (
    <div style={{
      width: '100%', height: '100%', background: 'var(--cream)', color: 'var(--ink)',
      position: 'relative', overflow: 'hidden', fontFamily: 'var(--body)'
    }}>
      {/* staff lines stretching across the canvas */}
      <svg width="100%" height="100%" viewBox="0 0 1280 640" preserveAspectRatio="xMidYMid slice" style={{ position: 'absolute', inset: 0, height: "640px", width: "1280px" }} fill="none">
        {[280, 320, 360, 400, 440].map((y, i) =>
        <line key={i} x1="0" y1={y} x2="1280" y2={y} stroke="var(--ink)" strokeWidth="1.2" opacity="0.18" />
        )}
        {/* bar lines */}
        <line x1="160" y1="280" x2="160" y2="440" stroke="var(--ink)" strokeWidth="2" opacity="0.4" />
        <line x1="1140" y1="280" x2="1140" y2="440" stroke="var(--ink)" strokeWidth="2" opacity="0.4" />
        {/* the slur arcing over the staff */}
        <path d="M260 240 C 360 100, 880 100, 980 240" stroke="var(--clay)" strokeWidth="3" strokeLinecap="round" fill="none" />
        <circle cx="260" cy="270" r="18" fill="var(--ink)" />
        <circle cx="980" cy="270" r="18" fill="var(--clay)" />
        {/* tempo marking */}
        <text x="160" y="240" style={{ fontFamily: 'var(--serif)', fontStyle: 'italic', fontSize: 26, strokeWidth: "1px" }} fill="var(--ink)" opacity="0.7">
          ♩ = 64 · adagio
        </text>
      </svg>

      {/* eyebrow top-left */}
      <div style={{ position: 'absolute', top: 32, left: 56, display: 'flex', alignItems: 'center', gap: 10 }}>
        <span style={{ width: 7, height: 7, borderRadius: 1, background: 'var(--clay)' }} />
        <span style={{ fontFamily: 'var(--mono)', fontSize: 13, letterSpacing: '0.06em', textTransform: 'uppercase', color: 'var(--ink-muted)' }}>
          for nextcloud
        </span>
      </div>

      {/* version top-right */}
      <div style={{ position: 'absolute', top: 32, right: 56, fontFamily: 'var(--mono)', fontSize: 13, color: 'var(--ink-muted)', letterSpacing: '0.04em' }}>
        v1.0 — allegretto
      </div>

      {/* wordmark + tagline below the staff */}
      <div style={{ position: 'absolute', bottom: 56, left: 56, right: 56, display: 'flex', alignItems: 'flex-end', justifyContent: 'space-between' }}>
        <div>
          <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 128, lineHeight: 0.92, letterSpacing: '-0.06em' }}>
            adagio
          </div>
          <div style={{ marginTop: 12, fontFamily: 'var(--serif)', fontStyle: 'italic', fontSize: 24, color: 'var(--ink-soft)', letterSpacing: '-0.01em' }}>
            the unhurried way to keep files in sync
          </div>
        </div>
        <div style={{ textAlign: 'right', fontFamily: 'var(--mono)', fontSize: 12, color: 'var(--ink-muted)', lineHeight: 1.7 }}>
          <div>AGPL-3.0</div>
          <div>linux · macOS · windows</div>
          <div>4.2 MB · single binary</div>
        </div>
      </div>
    </div>);

}

Object.assign(window, { BannerComposition, BannerGesture, BannerScore });