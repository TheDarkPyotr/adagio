import React, { useState } from 'react';
import { Icon } from './shared';
import type { IconName } from './shared';

function NewItemBtn({ icon, label, desc }: { icon: IconName; label: string; desc: string }) {
  const [h, setH] = useState(false);
  return (
    <button
      onMouseEnter={() => setH(true)} onMouseLeave={() => setH(false)}
      style={{ flex: 1, background: h ? 'var(--paper-2)' : 'var(--paper)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', padding: 16, textAlign: 'left', cursor: 'pointer', display: 'flex', alignItems: 'flex-start', gap: 14, transition: 'background 0.15s' }}>
      <div style={{ width: 32, height: 32, borderRadius: 8, background: 'var(--cream-2)', display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--clay)', flexShrink: 0 }}>
        <Icon name={icon} size={18} strokeWidth={2} />
      </div>
      <div>
        <div style={{ fontSize: 14, fontWeight: 600, color: 'var(--ink)' }}>{label}</div>
        <div style={{ fontSize: 12, color: 'var(--ink-muted)', marginTop: 2, lineHeight: 1.3 }}>{desc}</div>
      </div>
    </button>
  );
}

export default function NewFileDialog({ onClose }: { onClose: () => void }) {
  const [dragActive, setDragActive] = useState(false);

  return (
    <div
      onClick={onClose}
      style={{ position: 'absolute', inset: 0, background: 'color-mix(in srgb, var(--ink) 40%, transparent)', backdropFilter: 'blur(6px)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 20 }}>
      <div onClick={e => e.stopPropagation()}
        style={{ width: 500, background: 'var(--cream)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-3)', boxShadow: 'var(--shadow-lg)', overflow: 'hidden' }}>

        {/* header */}
        <div style={{ padding: '20px 24px 18px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', borderBottom: '1px solid var(--hairline)' }}>
          <div>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.2em', textTransform: 'uppercase', color: 'var(--clay)', marginBottom: 6 }}>Create or upload</div>
            <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 24, lineHeight: 1.1, letterSpacing: '-0.035em' }}>New item</div>
          </div>
          <button onClick={onClose} style={{ background: 'transparent', border: 'none', cursor: 'pointer', color: 'var(--ink-muted)' }}>
            <Icon name="x" size={20} />
          </button>
        </div>

        <div style={{ padding: 24 }}>
          {/* create buttons */}
          <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 24 }}>
            <NewItemBtn icon="folder-plus" label="Folder" desc="Group related files" />
            <NewItemBtn icon="file-plus" label="Markdown" desc="Draft a new document" />
          </div>

          {/* upload zone */}
          <label style={{ fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.18em', textTransform: 'uppercase', color: 'var(--ink-muted)', display: 'block', marginBottom: 12 }}>Upload</label>
          <div
            onDragOver={e => { e.preventDefault(); setDragActive(true); }}
            onDragLeave={() => setDragActive(false)}
            onDrop={e => { e.preventDefault(); setDragActive(false); }}
            style={{ height: 180, border: dragActive ? '2.5px dashed var(--clay)' : '1.5px dashed var(--hairline)', background: dragActive ? 'var(--paper-2)' : 'var(--paper)', borderRadius: 'var(--r-3)', display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 12, transition: 'all 0.2s', cursor: 'pointer' }}>
            <div style={{ width: 44, height: 44, borderRadius: 22, background: 'var(--cream-2)', display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--ink-soft)' }}>
              <Icon name="upload" size={20} strokeWidth={2} />
            </div>
            <div style={{ textAlign: 'center' }}>
              <div style={{ fontSize: 15, fontWeight: 500 }}>Drop files here</div>
              <div style={{ fontSize: 13, color: 'var(--ink-muted)', marginTop: 4 }}>or click to browse your computer</div>
            </div>
          </div>
        </div>

        {/* footer */}
        <div style={{ padding: '16px 24px', background: 'var(--paper-2)', borderTop: '1px solid var(--hairline)', display: 'flex', justifyContent: 'flex-end', gap: 10 }}>
          <button onClick={onClose}
            style={{ background: 'transparent', border: '1px solid var(--hairline)', padding: '8px 20px', borderRadius: 'var(--r-pill)', fontSize: 13, cursor: 'pointer', color: 'var(--ink)' }}>
            Cancel
          </button>
          <button
            style={{ background: 'var(--ink)', color: 'var(--cream)', border: 'none', padding: '8px 24px', borderRadius: 'var(--r-pill)', fontSize: 13, fontWeight: 500, cursor: 'pointer' }}>
            Upload
          </button>
        </div>
      </div>
    </div>
  );
}
