import './mocks/tauri';
import { render, screen, act } from '@testing-library/react';
import { invoke } from '@tauri-apps/api/core';
import TrayPopover from '../components/TrayPopover';

const idleStatus = {
  status: 'idle' as const,
  active_file_count: 0,
  total_bytes: 0,
  transferred_bytes: 0,
  eta_seconds: null,
  last_sync_at: null,
};

describe('TrayPopover', () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_status') return Promise.resolve(idleStatus);
      return Promise.resolve([]);
    });
  });

  it('renders the adagio wordmark', async () => {
    await act(async () => { render(<TrayPopover />); });
    expect(screen.getByText('adagio')).toBeInTheDocument();
  });

  it('shows IN SYNC status by default (null status resolves to idle)', async () => {
    await act(async () => { render(<TrayPopover />); });
    expect(screen.getByText('IN SYNC')).toBeInTheDocument();
  });

  it('shows IN SYNC status when status is idle', async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_status') return Promise.resolve(idleStatus);
      return Promise.resolve([]);
    });
    await act(async () => { render(<TrayPopover />); });
    expect(screen.getByText('IN SYNC')).toBeInTheDocument();
  });

  it('shows PAUSED status when sync is paused', async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_status') return Promise.resolve({ ...idleStatus, status: 'paused' });
      return Promise.resolve([]);
    });
    await act(async () => { render(<TrayPopover />); });
    expect(screen.getByText('PAUSED')).toBeInTheDocument();
  });

  it('shows SYNCING status when actively syncing', async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_status') return Promise.resolve({ ...idleStatus, status: 'syncing', active_file_count: 3 });
      return Promise.resolve([]);
    });
    await act(async () => { render(<TrayPopover />); });
    expect(screen.getByText('SYNCING 3 FILES')).toBeInTheDocument();
  });

  it('renders Pause syncing action when not paused', async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_status') return Promise.resolve(idleStatus);
      return Promise.resolve([]);
    });
    await act(async () => { render(<TrayPopover />); });
    expect(screen.getByText('Pause syncing')).toBeInTheDocument();
  });

  it('renders Resume syncing action when paused', async () => {
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_status') return Promise.resolve({ ...idleStatus, status: 'paused' });
      return Promise.resolve([]);
    });
    await act(async () => { render(<TrayPopover />); });
    expect(screen.getByText('Resume syncing')).toBeInTheDocument();
  });

  it('renders the four quick action labels', async () => {
    await act(async () => { render(<TrayPopover />); });
    expect(screen.getByText('Open Adagio folder')).toBeInTheDocument();
    expect(screen.getByText('Open in browser')).toBeInTheDocument();
    expect(screen.getByText('Pause syncing')).toBeInTheDocument();
    expect(screen.getByText('Preferences…')).toBeInTheDocument();
  });

  it('renders the Quit Adagio footer button', async () => {
    await act(async () => { render(<TrayPopover />); });
    expect(screen.getByText('Quit Adagio')).toBeInTheDocument();
  });
});
