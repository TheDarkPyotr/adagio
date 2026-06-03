import React, { useState } from 'react';
import type { ConflictDto } from '../tauri';

const KIND_LABELS: Record<string, string> = {
  content_modified: 'Content modified on both sides',
  renamed_both_sides: 'Renamed on both sides',
  deleted_with_content: 'Deleted locally / New content on server',
};

function formatBytes(n: number): string {
  if (n === 0) return '0 B';
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

function formatDate(iso: string): string {
  try {
    return new Date(iso).toLocaleString();
  } catch {
    return iso;
  }
}

interface Props {
  conflicts: ConflictDto[];
  onClose: () => void;
  resolveConflict: (id: string, side: 'local' | 'remote' | 'both') => Promise<void>;
  dismissAll?: () => Promise<unknown>;
}

export default function ConflictWizard({ conflicts, onClose, resolveConflict, dismissAll }: Props) {
  const [index, setIndex] = useState(0);
  const [resolving, setResolving] = useState(false);
  const [dismissing, setDismissing] = useState(false);

  const current = conflicts[index];
  if (!current) return null;

  const fileName = current.path.split('/').filter(Boolean).pop() ?? current.path;
  const total = conflicts.length;
  const isFolder = current.is_dir;
  const isDeletedWithContent = current.conflict_kind === 'deleted_with_content';

  const handleDismissAll = async () => {
    if (!dismissAll) return;
    setDismissing(true);
    try {
      await dismissAll();
      onClose();
    } finally {
      setDismissing(false);
    }
  };

  const handleResolve = async (side: 'local' | 'remote' | 'both') => {
    setResolving(true);
    try {
      await resolveConflict(current.id, side);
      const next = index + 1;
      if (next >= total) {
        onClose();
      } else {
        setIndex(next);
      }
    } finally {
      setResolving(false);
    }
  };

  return (
    <div
      style={{
        position: 'fixed', inset: 0,
        background: 'rgba(0,0,0,0.45)',
        display: 'flex', alignItems: 'center', justifyContent: 'center',
        zIndex: 1000,
        fontFamily: 'var(--body)',
      }}
    >
      <div
        style={{
          background: 'var(--paper)',
          borderRadius: 'var(--r-3)',
          padding: '28px 32px',
          width: 520,
          maxWidth: '90vw',
          boxShadow: '0 8px 32px rgba(0,0,0,0.18)',
          color: 'var(--ink)',
          position: 'relative',
        }}
        onClick={(e) => e.stopPropagation()}
      >
        {/* Header */}
        <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between', marginBottom: 20 }}>
          <div>
            <span style={{ fontSize: 13, color: 'var(--ink-muted)', fontWeight: 500 }}>
              Conflict {index + 1} of {total}
            </span>
            <h2 style={{ margin: '4px 0 0', fontSize: 17, fontWeight: 600 }}>{fileName}</h2>
          </div>
          <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
            {total > 1 && dismissAll && (
              <button
                data-testid="dismiss-all-btn"
                disabled={dismissing || resolving}
                onClick={handleDismissAll}
                style={{ background: 'none', border: '1px solid var(--hairline)', borderRadius: 'var(--r-2)', fontSize: 12, cursor: 'pointer', color: 'var(--ink-muted)', padding: '4px 10px' }}
              >
                {dismissing ? 'Dismissing…' : `Dismiss all ${total}`}
              </button>
            )}
            <button
              data-testid="dismiss-btn"
              onClick={onClose}
              style={{ background: 'none', border: 'none', fontSize: 20, cursor: 'pointer', color: 'var(--ink-muted)', padding: '4px 8px' }}
            >
              ×
            </button>
          </div>
        </div>

        {/* Path — only shown when there is a directory prefix */}
        {current.path !== fileName && (
          <p style={{ fontSize: 12, color: 'var(--ink-muted)', margin: '0 0 16px', wordBreak: 'break-all' }}>
            {current.path}
          </p>
        )}

        {/* Folder conflict kind label */}
        {isFolder && (
          <div style={{ marginBottom: 16, padding: '8px 12px', background: 'var(--cream)', borderRadius: 'var(--r-2)', fontSize: 13, fontWeight: 500 }}>
            {KIND_LABELS[current.conflict_kind] ?? current.conflict_kind}
          </div>
        )}

        {/* Impact warning for DeletedWithContent */}
        {isDeletedWithContent && (
          <div data-testid="impact-warning" style={{ marginBottom: 16, padding: '10px 14px', background: '#fff3cd', borderRadius: 'var(--r-2)', fontSize: 13, color: '#856404' }}>
            <strong>Warning:</strong> Choosing "Keep Server Version" will restore files to disk.
          </div>
        )}

        {/* Metadata comparison */}
        <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12, marginBottom: 24 }}>
          <div style={{ padding: '12px 14px', background: 'var(--cream)', borderRadius: 'var(--r-2)' }}>
            <div style={{ fontSize: 11, fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.08em', color: 'var(--ink-muted)', marginBottom: 8 }}>This device</div>
            <div style={{ fontSize: 13 }}>{formatBytes(current.local_size)}</div>
            <div style={{ fontSize: 12, color: 'var(--ink-muted)', marginTop: 4 }}>{formatDate(current.local_mtime)}</div>
          </div>
          <div style={{ padding: '12px 14px', background: 'var(--cream)', borderRadius: 'var(--r-2)' }}>
            <div style={{ fontSize: 11, fontWeight: 600, textTransform: 'uppercase', letterSpacing: '0.08em', color: 'var(--ink-muted)', marginBottom: 8 }}>Cloud copy</div>
            <div style={{ fontSize: 13 }}>{formatBytes(current.remote_size)}</div>
            <div style={{ fontSize: 12, color: 'var(--ink-muted)', marginTop: 4 }}>{formatDate(current.remote_mtime)}</div>
          </div>
        </div>

        {/* Spinner overlay while resolving */}
        {resolving && (
          <div data-testid="resolution-spinner" style={{ position: 'absolute', inset: 0, background: 'rgba(255,255,255,0.75)', display: 'flex', alignItems: 'center', justifyContent: 'center', borderRadius: 'var(--r-3)' }}>
            <div style={{ width: 28, height: 28, border: '3px solid var(--hairline)', borderTopColor: 'var(--ink)', borderRadius: '50%', animation: 'spin 0.7s linear infinite' }} />
          </div>
        )}

        {/* Resolution buttons */}
        <div style={{ display: 'flex', gap: 10 }}>
          <button
            disabled={resolving}
            onClick={() => handleResolve('local')}
            style={{ flex: 1, padding: '10px 0', background: 'var(--clay)', color: '#fff', border: 'none', borderRadius: 'var(--r-2)', fontSize: 13, fontWeight: 600, cursor: 'pointer' }}
          >
            Keep Local Version
          </button>
          <button
            disabled={resolving}
            onClick={() => handleResolve('remote')}
            style={{ flex: 1, padding: '10px 0', background: 'var(--forest)', color: '#fff', border: 'none', borderRadius: 'var(--r-2)', fontSize: 13, fontWeight: 600, cursor: 'pointer' }}
          >
            Keep Server Version
          </button>
          <button
            disabled={resolving}
            onClick={() => handleResolve('both')}
            style={{ flex: 1, padding: '10px 0', background: 'var(--ink)', color: '#fff', border: 'none', borderRadius: 'var(--r-2)', fontSize: 13, fontWeight: 600, cursor: 'pointer' }}
          >
            Keep Both
          </button>
        </div>
      </div>
    </div>
  );
}
