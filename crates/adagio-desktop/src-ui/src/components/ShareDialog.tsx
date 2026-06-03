import React, { useState, useCallback, useRef } from 'react';
import { Icon } from './shared';

interface Person { name: string; role: 'editor' | 'viewer' | 'commenter' }

function Segment({ label, value, setValue, options }: {
  label: string;
  value: string;
  setValue: (v: string) => void;
  options: [string, string][];
}) {
  return (
    <div>
      <label style={MONO_LABEL}>{label}</label>
      <div style={{ marginTop: 8, display: 'flex', background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', padding: 2 }}>
        {options.map(([id, lbl]) => (
          <button key={id} onClick={() => setValue(id)}
            style={{ flex: 1, background: value === id ? 'var(--ink)' : 'transparent', color: value === id ? 'var(--cream)' : 'var(--ink-soft)', border: 'none', padding: '6px 8px', borderRadius: 4, fontSize: 12, fontWeight: 500, cursor: 'pointer' }}>
            {lbl}
          </button>
        ))}
      </div>
    </div>
  );
}

export default function ShareDialog({ path, onClose }: { path: string; onClose: () => void }) {
  const fileName = path.split('/').filter(Boolean).pop() ?? path;
  const [expiry, setExpiry] = useState('7d');
  const [perm, setPerm] = useState('view');
  const [pwd, setPwd] = useState(false);
  const [hideDownload, setHideDownload] = useState(true);
  const [notifyOnOpen, setNotifyOnOpen] = useState(false);
  const [people, setPeople] = useState<Person[]>([]);
  const [inputVal, setInputVal] = useState('');
  const [copied, setCopied] = useState(false);
  const [pwdValue, setPwdValue] = useState('');
  const copyTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  const handleCopy = useCallback(() => {
    navigator.clipboard.writeText('https://example.com/share/link').catch(() => {});
    setCopied(true);
    if (copyTimeoutRef.current) clearTimeout(copyTimeoutRef.current);
    copyTimeoutRef.current = setTimeout(() => setCopied(false), 2000);
  }, []);

  const removePerson = useCallback((i: number) => {
    setPeople(prev => prev.filter((_, j) => j !== i));
  }, []);

  const handleKeyDown = useCallback((e: React.KeyboardEvent<HTMLInputElement>) => {
    if ((e.key === 'Enter' || e.key === ',') && inputVal.trim()) {
      setPeople(prev => [...prev, { name: inputVal.trim(), role: 'viewer' }]);
      setInputVal('');
      e.preventDefault();
    }
  }, [inputVal]);

  return (
    <div
      onClick={onClose}
      style={{ position: 'absolute', inset: 0, background: 'color-mix(in srgb, var(--ink) 40%, transparent)', backdropFilter: 'blur(6px)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 20 }}>
      <div onClick={e => e.stopPropagation()}
        style={{ width: 540, background: 'var(--cream)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-3)', boxShadow: 'var(--shadow-lg)', overflow: 'hidden' }}>

        {/* header */}
        <div style={{ padding: '20px 24px 14px', display: 'flex', alignItems: 'flex-start', justifyContent: 'space-between', borderBottom: '1px solid var(--hairline)' }}>
          <div>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.2em', textTransform: 'uppercase', color: 'var(--clay)', marginBottom: 6 }}>Share</div>
            <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 22, lineHeight: 1.1, letterSpacing: '-0.035em' }}>{fileName}</div>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 11, color: 'var(--ink-muted)', marginTop: 4 }}>{path}</div>
          </div>
          <button onClick={onClose} style={{ background: 'transparent', border: 'none', cursor: 'pointer', color: 'var(--ink-muted)', flexShrink: 0 }}>
            <Icon name="x" size={18} />
          </button>
        </div>

        <div style={{ padding: '18px 24px', display: 'flex', flexDirection: 'column', gap: 16 }}>
          {/* recipients */}
          <div>
            <label style={MONO_LABEL}>With</label>
            <div style={{ marginTop: 8, padding: '6px 8px 8px', background: 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)' }}>
              <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6, alignItems: 'center' }}>
                {people.map((p, i) => (
                  <span key={i} style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '4px 6px 4px 4px', background: 'var(--cream-2)', borderRadius: 'var(--r-pill)', fontSize: 12 }}>
                    <span style={{ width: 18, height: 18, borderRadius: 9, background: 'var(--forest)', color: 'var(--cream)', fontSize: 9, display: 'flex', alignItems: 'center', justifyContent: 'center', fontWeight: 600 }}>
                      {p.name.split(' ').map(n => n[0]).join('').slice(0, 2)}
                    </span>
                    {p.name}
                    <span style={{ color: 'var(--ink-muted)', fontFamily: 'var(--mono)', fontSize: 10, paddingLeft: 4, borderLeft: '1px solid var(--hairline)' }}>{p.role}</span>
                    <button onClick={() => removePerson(i)} style={{ background: 'transparent', border: 'none', cursor: 'pointer', color: 'var(--ink-muted)', padding: 0, marginLeft: 2, display: 'flex' }}>
                      <Icon name="x" size={12} />
                    </button>
                  </span>
                ))}
                <input
                  value={inputVal}
                  onChange={e => setInputVal(e.target.value)}
                  onKeyDown={handleKeyDown}
                  placeholder="Add a name or email…"
                  style={{ flex: 1, minWidth: 140, border: 'none', background: 'transparent', outline: 'none', fontSize: 13, padding: '4px 6px', fontFamily: 'var(--body)' }}
                />
              </div>
            </div>
          </div>

          {/* permission + expiry */}
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 14 }}>
            <Segment label="Can" value={perm} setValue={setPerm} options={[['view', 'View'], ['comment', 'Comment'], ['edit', 'Edit']]} />
            <Segment label="Until" value={expiry} setValue={setExpiry} options={[['24h', '24 h'], ['7d', '7 days'], ['30d', '30 days'], ['never', 'No limit']]} />
          </div>

          {/* link */}
          <div>
            <label style={MONO_LABEL}>Or, a link</label>
            <div style={{ marginTop: 8, display: 'flex', alignItems: 'center', gap: 8, background: 'var(--ink)', color: 'var(--cream)', padding: '10px 12px', borderRadius: 'var(--r-2)', fontFamily: 'var(--mono)', fontSize: 12 }}>
              <Icon name="link" size={14} color="var(--clay-soft)" />
              <span style={{ flex: 1, whiteSpace: 'nowrap', overflow: 'hidden', textOverflow: 'ellipsis' }}>Generating link…</span>
              <button onClick={handleCopy} style={{ background: 'color-mix(in srgb, var(--cream) 12%, transparent)', color: copied ? 'var(--good)' : 'var(--cream)', border: 'none', padding: '4px 10px', borderRadius: 4, fontSize: 11, cursor: 'pointer', fontFamily: 'inherit', transition: 'color 0.15s', display: 'flex', alignItems: 'center', gap: 4 }}>
                {copied ? <><Icon name="check" size={11} color="var(--good)" /> copied</> : 'copy'}
              </button>
            </div>
            {pwd && (
              <input
                type="password"
                placeholder="Set a password…"
                value={pwdValue}
                onChange={e => setPwdValue(e.target.value)}
                style={{ marginTop: 8, width: '100%', boxSizing: 'border-box', padding: '8px 12px', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', background: 'var(--paper)', fontFamily: 'var(--mono)', fontSize: 13, color: 'var(--ink)', outline: 'none' }}
              />
            )}
            <div style={{ marginTop: 10, display: 'flex', alignItems: 'center', gap: 16, fontSize: 12.5, color: 'var(--ink-soft)' }}>
              <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer' }}>
                <input type="checkbox" checked={pwd} onChange={() => setPwd(!pwd)} style={{ accentColor: 'var(--ink)' }}/> Password protect
              </label>
              <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer' }}>
                <input type="checkbox" checked={hideDownload} onChange={() => setHideDownload(!hideDownload)} style={{ accentColor: 'var(--ink)' }}/> Hide download
              </label>
              <label style={{ display: 'flex', alignItems: 'center', gap: 6, cursor: 'pointer' }}>
                <input type="checkbox" checked={notifyOnOpen} onChange={() => setNotifyOnOpen(!notifyOnOpen)} style={{ accentColor: 'var(--ink)' }}/> Notify on open
              </label>
            </div>
          </div>

          {/* note */}
          <div>
            <label style={MONO_LABEL}>A note (optional)</label>
            <textarea placeholder="Add a message…"
              style={{ marginTop: 8, width: '100%', minHeight: 56, padding: 12, border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', background: 'var(--paper)', fontFamily: 'var(--body)', fontSize: 13, color: 'var(--ink)', resize: 'none', outline: 'none', boxSizing: 'border-box' }}
            />
          </div>
        </div>

        {/* footer */}
        <div style={{ padding: '14px 24px', background: 'var(--paper-2)', borderTop: '1px solid var(--hairline)', display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
          <div style={{ fontSize: 12, color: 'var(--ink-muted)', display: 'flex', alignItems: 'center', gap: 6 }}>
            <Icon name="shield" size={13} color="var(--forest)" /> End-to-end encrypted folder
          </div>
          <div style={{ display: 'flex', gap: 8 }}>
            <button onClick={onClose}
              style={{ background: 'transparent', border: '1px solid var(--hairline)', padding: '8px 16px', borderRadius: 'var(--r-pill)', fontSize: 13, cursor: 'pointer', color: 'var(--ink)' }}>
              Cancel
            </button>
            <button
              style={{ background: 'var(--ink)', color: 'var(--cream)', border: 'none', padding: '8px 18px', borderRadius: 'var(--r-pill)', fontSize: 13, fontWeight: 500, cursor: 'pointer' }}>
              {people.length > 0 ? `Share with ${people.length} ${people.length === 1 ? 'person' : 'people'}` : 'Create link'}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

const MONO_LABEL: React.CSSProperties = {
  fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.18em',
  textTransform: 'uppercase', color: 'var(--ink-muted)',
};
