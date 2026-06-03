import './mocks/tauri';
import { render, screen, fireEvent, waitFor, act } from '@testing-library/react';
import ConflictWizard from '../components/ConflictWizard';

// Minimal ConflictDto fixture helper.
function makeConflict(overrides: Partial<ConflictDtoLike> = {}): ConflictDtoLike {
  return {
    id: 'conflict-1',
    pair_id: 'pair-1',
    path: 'docs/report.pdf',
    local_mtime: '2024-03-15T10:00:00Z',
    remote_mtime: '2024-03-15T11:00:00Z',
    local_size: 1024,
    remote_size: 2048,
    policy: 'ask',
    resolution: null,
    detected_at: '2024-03-15T09:00:00Z',
    resolved_at: null,
    is_dir: false,
    conflict_kind: 'content_modified',
    ...overrides,
  };
}

// Shape used until we import the real type.
interface ConflictDtoLike {
  id: string;
  pair_id: string;
  path: string;
  local_mtime: string;
  remote_mtime: string;
  local_size: number;
  remote_size: number;
  policy: string;
  resolution: string | null;
  detected_at: string;
  resolved_at: string | null;
  is_dir: boolean;
  conflict_kind: string;
}

describe('ConflictWizard', () => {
  const onClose = vi.fn();
  const resolveConflict = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    resolveConflict.mockResolvedValue(undefined);
  });

  // T020 — renders file name and local vs server metadata.
  it('renders conflict path and local vs server metadata', () => {
    render(
      <ConflictWizard
        conflicts={[makeConflict()]}
        onClose={onClose}
        resolveConflict={resolveConflict}
      />
    );
    expect(screen.getByText('report.pdf')).toBeInTheDocument();
    expect(screen.getByText(/local/i)).toBeInTheDocument();
    expect(screen.getByText(/server/i)).toBeInTheDocument();
  });

  // T021 — calls resolveConflict("local") when Keep Local Version is clicked.
  it('calls resolveConflict("local") when Keep Local Version is clicked', async () => {
    render(
      <ConflictWizard
        conflicts={[makeConflict()]}
        onClose={onClose}
        resolveConflict={resolveConflict}
      />
    );
    fireEvent.click(screen.getByText(/keep local/i));
    expect(resolveConflict).toHaveBeenCalledWith('conflict-1', 'local');
  });

  // T022 — calls resolveConflict("remote") when Keep Server Version is clicked.
  it('calls resolveConflict("remote") when Keep Server Version is clicked', async () => {
    render(
      <ConflictWizard
        conflicts={[makeConflict()]}
        onClose={onClose}
        resolveConflict={resolveConflict}
      />
    );
    fireEvent.click(screen.getByText(/keep server/i));
    expect(resolveConflict).toHaveBeenCalledWith('conflict-1', 'remote');
  });

  // T023 — calls resolveConflict("both") when Keep Both is clicked.
  it('calls resolveConflict("both") when Keep Both is clicked', async () => {
    render(
      <ConflictWizard
        conflicts={[makeConflict()]}
        onClose={onClose}
        resolveConflict={resolveConflict}
      />
    );
    fireEvent.click(screen.getByText(/keep both/i));
    expect(resolveConflict).toHaveBeenCalledWith('conflict-1', 'both');
  });

  // T024 — shows loading spinner while resolution is in-flight.
  it('shows loading spinner while resolution is in-flight', async () => {
    let resolve: () => void;
    resolveConflict.mockImplementation(
      () => new Promise<void>((res) => { resolve = res; })
    );

    render(
      <ConflictWizard
        conflicts={[makeConflict()]}
        onClose={onClose}
        resolveConflict={resolveConflict}
      />
    );

    fireEvent.click(screen.getByText(/keep local/i));
    expect(screen.getByTestId('resolution-spinner')).toBeInTheDocument();

    await act(async () => { resolve!(); });
  });

  // T025 — shows conflict_kind label for folder conflicts.
  it('shows conflict_kind label for folder conflicts', () => {
    render(
      <ConflictWizard
        conflicts={[makeConflict({ is_dir: true, conflict_kind: 'renamed_both_sides' })]}
        onClose={onClose}
        resolveConflict={resolveConflict}
      />
    );
    // Should display the kind in a human-readable form.
    expect(screen.getByText(/renamed/i)).toBeInTheDocument();
  });

  // T026 — shows impact warning with file list for DeletedWithContent kind.
  it('shows impact warning with file list for DeletedWithContent kind', () => {
    render(
      <ConflictWizard
        conflicts={[makeConflict({ is_dir: true, conflict_kind: 'deleted_with_content' })]}
        onClose={onClose}
        resolveConflict={resolveConflict}
      />
    );
    expect(screen.getByTestId('impact-warning')).toBeInTheDocument();
  });

  // T037 — shows "Conflict N of M" step counter.
  it('shows "Conflict N of M" step counter', () => {
    const conflicts = [makeConflict({ id: 'c1' }), makeConflict({ id: 'c2' })];
    render(
      <ConflictWizard
        conflicts={conflicts}
        onClose={onClose}
        resolveConflict={resolveConflict}
      />
    );
    expect(screen.getByText(/1 of 2/i)).toBeInTheDocument();
  });

  // T038 — auto-advances to next conflict after successful resolution.
  it('auto-advances to next conflict after resolution', async () => {
    const conflicts = [
      makeConflict({ id: 'c1', path: 'first.txt' }),
      makeConflict({ id: 'c2', path: 'second.txt' }),
    ];

    render(
      <ConflictWizard
        conflicts={conflicts}
        onClose={onClose}
        resolveConflict={resolveConflict}
      />
    );

    expect(screen.getByText('first.txt')).toBeInTheDocument();
    fireEvent.click(screen.getByText(/keep local/i));

    await waitFor(() => {
      expect(screen.getByText('second.txt')).toBeInTheDocument();
    });
  });

  // T039 — closes and calls onClose when last conflict is resolved.
  it('closes when last conflict is resolved', async () => {
    render(
      <ConflictWizard
        conflicts={[makeConflict()]}
        onClose={onClose}
        resolveConflict={resolveConflict}
      />
    );
    fireEvent.click(screen.getByText(/keep local/i));
    await waitFor(() => expect(onClose).toHaveBeenCalledTimes(1));
  });

  // T043 — dismiss button closes wizard without calling resolveConflict.
  it('dismiss button closes wizard without calling resolveConflict', () => {
    render(
      <ConflictWizard
        conflicts={[makeConflict()]}
        onClose={onClose}
        resolveConflict={resolveConflict}
      />
    );
    fireEvent.click(screen.getByTestId('dismiss-btn'));
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(resolveConflict).not.toHaveBeenCalled();
  });
});
