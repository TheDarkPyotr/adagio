import './mocks/tauri';
import { render, screen, fireEvent, act } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import ActivityScene from '../components/ActivityScene';

describe('ActivityScene', () => {
  beforeEach(() => vi.clearAllMocks());

  it('renders the Activity heading', () => {
    render(<ActivityScene />);
    expect(screen.getByText('Activity')).toBeInTheDocument();
  });

  it('renders all five filter pills', () => {
    render(<ActivityScene />);
    expect(screen.getByText('All')).toBeInTheDocument();
    expect(screen.getByText('Edits')).toBeInTheDocument();
    expect(screen.getByText('Shares')).toBeInTheDocument();
    expect(screen.getByText('Sync')).toBeInTheDocument();
    expect(screen.getByText('Conflicts')).toBeInTheDocument();
  });

  it('shows 0 events on initial render with empty log', async () => {
    await act(async () => { render(<ActivityScene />); });
    expect(screen.getByText(/0 events/)).toBeInTheDocument();
  });

  it('calls getActivityLog with edit filter when Edits pill is clicked', async () => {
    await act(async () => { render(<ActivityScene />); });
    fireEvent.click(screen.getByText('Edits'));
    expect(vi.mocked(invoke)).toHaveBeenCalledWith('get_activity_log', expect.objectContaining({ filter: 'edit' }));
  });

  it('calls getActivityLog with no filter when All pill is clicked', async () => {
    await act(async () => { render(<ActivityScene />); });
    fireEvent.click(screen.getByText('Conflicts'));
    vi.mocked(invoke).mockClear();
    fireEvent.click(screen.getByText('All'));
    expect(vi.mocked(invoke)).toHaveBeenCalledWith('get_activity_log', expect.objectContaining({ filter: undefined }));
  });

  it('renders Today bucket when entries exist for today', async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_activity_log') return Promise.resolve([{
        at: Date.now() - 30_000,
        kind: 'edit',
        path: '/notes.md',
        account_id: 'a1',
      }]);
      return Promise.resolve([]);
    });
    await act(async () => { render(<ActivityScene />); });
    expect(screen.getByText('Today')).toBeInTheDocument();
  });

  it('shows correct event count with data', async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_activity_log') return Promise.resolve([
        { at: Date.now() - 1000, kind: 'edit', path: '/a.md', account_id: 'a1' },
        { at: Date.now() - 2000, kind: 'sync', path: '/b.md', account_id: 'a1' },
      ]);
      return Promise.resolve([]);
    });
    await act(async () => { render(<ActivityScene />); });
    expect(screen.getByText(/2 events/)).toBeInTheDocument();
  });
});
