import React, { useEffect, useRef, useState } from 'react';
import { Icon } from './shared';
import type { IconName } from './shared';
import { createLocalFolder, createLocalFile, writeLocalFiles } from '../tauri';
import type { UploadEntry } from '../tauri';

// ── Shared sub-components ─────────────────────────────────────────────────────

function NewItemBtn({
  icon, label, desc, onClick,
}: { icon: IconName; label: string; desc: string; onClick: () => void }) {
  const [h, setH] = useState(false);
  return (
    <button
      onClick={onClick}
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

// ── File reading helpers ──────────────────────────────────────────────────────

async function readFileAsBytes(file: File): Promise<UploadEntry> {
  const buf = await file.arrayBuffer();
  return {
    name: file.name,
    content: Array.from(new Uint8Array(buf)),
  };
}

/** From a folder-upload file list, extract relative sub-directories. */
async function readFilesWithPaths(fileList: FileList): Promise<UploadEntry[]> {
  const entries: UploadEntry[] = [];
  for (let i = 0; i < fileList.length; i++) {
    const file = fileList[i];
    const buf = await file.arrayBuffer();
    // webkitRelativePath = "FolderName/sub/file.txt"
    const rel = (file as File & { webkitRelativePath?: string }).webkitRelativePath ?? '';
    const parts = rel.split('/');
    // Drop the file name (last part) and the top-level folder name (first part).
    const subpath = parts.length > 2 ? parts.slice(1, -1).join('/') : undefined;
    entries.push({
      name: file.name,
      relSubpath: subpath,
      content: Array.from(new Uint8Array(buf)),
    });
  }
  return entries;
}

// ── Pending upload list ───────────────────────────────────────────────────────

interface PendingFile {
  entry: UploadEntry;
  sizeMb: number;
}

function PendingFileRow({ name, sizeMb, onRemove }: { name: string; sizeMb: number; onRemove: () => void }) {
  return (
    <div style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '6px 0', borderBottom: '1px solid var(--hairline)' }}>
      <Icon name="file" size={14} color="var(--ink-muted)" />
      <span style={{ flex: 1, fontSize: 13, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{name}</span>
      <span style={{ fontSize: 11, color: 'var(--ink-muted)', flexShrink: 0 }}>{sizeMb < 0.1 ? '<0.1' : sizeMb.toFixed(1)} MB</span>
      <button onClick={onRemove} style={{ background: 'none', border: 'none', cursor: 'pointer', color: 'var(--ink-muted)', padding: 2 }}>
        <Icon name="x" size={13} />
      </button>
    </div>
  );
}

// ── Main dialog ───────────────────────────────────────────────────────────────

type Mode = 'idle' | 'naming-folder' | 'naming-md';

export default function NewFileDialog({
  onClose,
  onCreated,
  localRoot,
  currentPath,
}: {
  onClose: () => void;
  onCreated?: () => void;
  localRoot: string;
  currentPath: string;
}) {
  const [mode, setMode] = useState<Mode>('idle');
  const [name, setName] = useState('');
  const [dragActive, setDragActive] = useState(false);
  const [pending, setPending] = useState<PendingFile[]>([]);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fileInputRef = useRef<HTMLInputElement>(null);
  const folderInputRef = useRef<HTMLInputElement>(null);
  const nameInputRef = useRef<HTMLInputElement>(null);

  // React strips non-standard DOM attributes, so set webkitdirectory imperatively.
  useEffect(() => {
    if (folderInputRef.current) {
      folderInputRef.current.setAttribute('webkitdirectory', '');
      folderInputRef.current.setAttribute('directory', '');
      folderInputRef.current.setAttribute('multiple', '');
    }
  }, []);

  // ── Create folder ─────────────────────────────────────────────────────────

  const startNaming = (m: Mode) => {
    setMode(m);
    setName(m === 'naming-md' ? 'Untitled.md' : 'New folder');
    setError(null);
    setTimeout(() => { nameInputRef.current?.focus(); nameInputRef.current?.select(); }, 50);
  };

  const handleCreate = async () => {
    const trimmed = name.trim();
    if (!trimmed) { setError('Name cannot be empty.'); return; }
    if (trimmed.includes('/') || trimmed.includes('\\')) { setError('Name cannot contain slashes.'); return; }
    setBusy(true);
    setError(null);
    try {
      if (mode === 'naming-folder') {
        await createLocalFolder(localRoot, currentPath, trimmed);
      } else {
        const finalName = trimmed.endsWith('.md') ? trimmed : trimmed + '.md';
        await createLocalFile(localRoot, currentPath, finalName, `# ${finalName.replace(/\.md$/, '')}\n`);
      }
      onCreated?.();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  // ── File / folder upload ──────────────────────────────────────────────────

  const addFiles = async (fileList: FileList, isFolder = false) => {
    setError(null);
    const entries = isFolder ? await readFilesWithPaths(fileList) : await Promise.all(
      Array.from(fileList).map(readFileAsBytes),
    );
    const newPending: PendingFile[] = entries.map(e => ({
      entry: e,
      sizeMb: e.content.length / 1_048_576,
    }));
    setPending(prev => {
      const existingNames = new Set(prev.map(p => p.entry.name + (p.entry.relSubpath ?? '')));
      return [...prev, ...newPending.filter(p => !existingNames.has(p.entry.name + (p.entry.relSubpath ?? '')))];
    });
    if (mode === 'idle') setMode('idle'); // stay on upload view
  };

  const handleDrop = async (e: React.DragEvent) => {
    e.preventDefault();
    setDragActive(false);
    if (e.dataTransfer.files.length > 0) await addFiles(e.dataTransfer.files);
  };

  const handleFileInput = async (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      await addFiles(e.target.files, false);
    }
    e.target.value = '';
  };

  const handleFolderInput = async (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files && e.target.files.length > 0) {
      await addFiles(e.target.files, true);
    }
    e.target.value = '';
  };

  const handleUpload = async () => {
    if (pending.length === 0) return;
    setBusy(true);
    setError(null);
    try {
      await writeLocalFiles(localRoot, currentPath, pending.map(p => p.entry));
      onCreated?.();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setBusy(false);
    }
  };

  // ── Render ────────────────────────────────────────────────────────────────

  const isNaming = mode === 'naming-folder' || mode === 'naming-md';
  const hasUploads = pending.length > 0;

  return (
    <div
      onClick={onClose}
      style={{ position: 'absolute', inset: 0, background: 'color-mix(in srgb, var(--ink) 40%, transparent)', backdropFilter: 'blur(6px)', display: 'flex', alignItems: 'center', justifyContent: 'center', zIndex: 20 }}>
      <div
        onClick={e => e.stopPropagation()}
        style={{ width: 500, background: 'var(--cream)', border: '1px solid var(--hairline)', borderRadius: 'var(--r-3)', boxShadow: 'var(--shadow-lg)', overflow: 'hidden' }}>

        {/* Header */}
        <div style={{ padding: '20px 24px 18px', display: 'flex', alignItems: 'center', justifyContent: 'space-between', borderBottom: '1px solid var(--hairline)' }}>
          <div>
            <div style={{ fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.2em', textTransform: 'uppercase', color: 'var(--clay)', marginBottom: 6 }}>
              {isNaming ? (mode === 'naming-folder' ? 'New folder' : 'New document') : 'Create or upload'}
            </div>
            <div style={{ fontFamily: 'var(--body)', fontWeight: 500, fontSize: 24, lineHeight: 1.1, letterSpacing: '-0.035em' }}>
              {isNaming ? 'Choose a name' : 'New item'}
            </div>
          </div>
          <button onClick={onClose} style={{ background: 'transparent', border: 'none', cursor: 'pointer', color: 'var(--ink-muted)' }}>
            <Icon name="x" size={20} />
          </button>
        </div>

        <div style={{ padding: 24 }}>

          {/* ── Naming form ── */}
          {isNaming && (
            <div>
              <div style={{ marginBottom: 8, fontSize: 13, color: 'var(--ink-muted)' }}>
                {mode === 'naming-folder' ? 'Folder name' : 'File name'}
              </div>
              <input
                ref={nameInputRef}
                value={name}
                onChange={e => { setName(e.target.value); setError(null); }}
                onKeyDown={e => { if (e.key === 'Enter') handleCreate(); if (e.key === 'Escape') { setMode('idle'); } }}
                placeholder={mode === 'naming-folder' ? 'New folder' : 'Untitled.md'}
                style={{ width: '100%', padding: '10px 14px', fontSize: 15, border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', background: 'var(--paper)', color: 'var(--ink)', outline: 'none', boxSizing: 'border-box', fontFamily: 'var(--body)' }}
              />
              {error && <div style={{ marginTop: 8, fontSize: 12, color: 'var(--danger)' }}>{error}</div>}
            </div>
          )}

          {/* ── Idle view: create buttons + upload zone ── */}
          {!isNaming && (
            <>
              {/* Create buttons */}
              <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 24 }}>
                <NewItemBtn icon="folder-plus" label="Folder" desc="Group related files" onClick={() => startNaming('naming-folder')} />
                <NewItemBtn icon="file-plus" label="Markdown" desc="Draft a new document" onClick={() => startNaming('naming-md')} />
              </div>

              {/* Upload zone */}
              <label style={{ fontFamily: 'var(--mono)', fontSize: 10, letterSpacing: '0.18em', textTransform: 'uppercase', color: 'var(--ink-muted)', display: 'block', marginBottom: 12 }}>Upload</label>

              {/* Drop area */}
              <div
                onDragOver={e => { e.preventDefault(); setDragActive(true); }}
                onDragLeave={() => setDragActive(false)}
                onDrop={handleDrop}
                onClick={() => fileInputRef.current?.click()}
                style={{ height: hasUploads ? 'auto' : 140, minHeight: hasUploads ? 0 : 140, border: dragActive ? '2.5px dashed var(--clay)' : '1.5px dashed var(--hairline)', background: dragActive ? 'var(--paper-2)' : 'var(--paper)', borderRadius: 'var(--r-3)', display: 'flex', flexDirection: 'column', alignItems: hasUploads ? 'stretch' : 'center', justifyContent: hasUploads ? 'flex-start' : 'center', gap: 12, transition: 'all 0.2s', cursor: 'pointer', padding: hasUploads ? '12px 14px' : 0, overflow: 'hidden' }}>

                {hasUploads ? (
                  <div onClick={e => e.stopPropagation()}>
                    {pending.map((p, i) => (
                      <PendingFileRow
                        key={`${p.entry.name}-${p.entry.relSubpath ?? ''}`}
                        name={p.entry.relSubpath ? `${p.entry.relSubpath}/${p.entry.name}` : p.entry.name}
                        sizeMb={p.sizeMb}
                        onRemove={() => setPending(prev => prev.filter((_, j) => j !== i))}
                      />
                    ))}
                    <button
                      onClick={() => fileInputRef.current?.click()}
                      style={{ marginTop: 10, background: 'none', border: 'none', cursor: 'pointer', fontSize: 12, color: 'var(--clay)', padding: 0, display: 'flex', alignItems: 'center', gap: 5 }}>
                      <Icon name="plus" size={13} /> Add more files
                    </button>
                  </div>
                ) : (
                  <>
                    <div style={{ width: 44, height: 44, borderRadius: 22, background: 'var(--cream-2)', display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--ink-soft)' }}>
                      <Icon name="upload" size={20} strokeWidth={2} />
                    </div>
                    <div style={{ textAlign: 'center' }}>
                      <div style={{ fontSize: 15, fontWeight: 500 }}>Drop files here</div>
                      <div style={{ fontSize: 13, color: 'var(--ink-muted)', marginTop: 4 }}>or click to browse your computer</div>
                    </div>
                  </>
                )}
              </div>

              {/* Upload folder button */}
              <button
                onClick={() => folderInputRef.current?.click()}
                style={{ marginTop: 10, background: 'none', border: 'none', cursor: 'pointer', fontSize: 13, color: 'var(--ink-muted)', padding: 0, display: 'flex', alignItems: 'center', gap: 6 }}>
                <Icon name="folder-plus" size={14} /> Upload entire folder
              </button>

              {error && <div style={{ marginTop: 10, fontSize: 12, color: 'var(--danger)' }}>{error}</div>}

              {/* Hidden inputs */}
              <input ref={fileInputRef} type="file" multiple style={{ display: 'none' }} onChange={handleFileInput} />
              <input ref={folderInputRef} type="file" style={{ display: 'none' }} onChange={handleFolderInput} />
            </>
          )}
        </div>

        {/* Footer */}
        <div style={{ padding: '16px 24px', background: 'var(--paper-2)', borderTop: '1px solid var(--hairline)', display: 'flex', justifyContent: 'space-between', alignItems: 'center', gap: 10 }}>
          <div>
            {isNaming && (
              <button
                onClick={() => setMode('idle')}
                style={{ background: 'transparent', border: 'none', cursor: 'pointer', fontSize: 13, color: 'var(--ink-muted)', padding: 0, display: 'flex', alignItems: 'center', gap: 5 }}>
                <span style={{ display: 'inline-block', transform: 'rotate(180deg)' }}><Icon name="chevron" size={14} /></span> Back
              </button>
            )}
          </div>
          <div style={{ display: 'flex', gap: 10 }}>
            <button
              onClick={onClose}
              style={{ background: 'transparent', border: '1px solid var(--hairline)', padding: '8px 20px', borderRadius: 'var(--r-pill)', fontSize: 13, cursor: 'pointer', color: 'var(--ink)' }}>
              Cancel
            </button>
            {isNaming ? (
              <button
                onClick={handleCreate}
                disabled={busy || !name.trim()}
                style={{ background: busy || !name.trim() ? 'var(--ink-soft)' : 'var(--ink)', color: 'var(--cream)', border: 'none', padding: '8px 24px', borderRadius: 'var(--r-pill)', fontSize: 13, fontWeight: 500, cursor: busy || !name.trim() ? 'not-allowed' : 'pointer' }}>
                {busy ? 'Creating…' : 'Create'}
              </button>
            ) : (
              <button
                onClick={handleUpload}
                disabled={busy || pending.length === 0}
                style={{ background: busy || pending.length === 0 ? 'var(--ink-soft)' : 'var(--ink)', color: 'var(--cream)', border: 'none', padding: '8px 24px', borderRadius: 'var(--r-pill)', fontSize: 13, fontWeight: 500, cursor: busy || pending.length === 0 ? 'not-allowed' : 'pointer' }}>
                {busy ? 'Uploading…' : `Upload${pending.length > 0 ? ` (${pending.length})` : ''}`}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
