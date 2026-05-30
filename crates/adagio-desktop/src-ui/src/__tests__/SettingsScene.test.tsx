import './mocks/tauri';
import { render, screen, fireEvent, waitFor } from '@testing-library/react';
import SettingsScene from '../components/SettingsScene';

const defaultProps = {
  palette: 'sienna',
  onPalette: vi.fn(),
  syncStatus: null,
  onBack: vi.fn(),
  onPairs: vi.fn(),
};

describe('SettingsScene', () => {
  beforeEach(() => vi.resetAllMocks());

  it('renders the Appearance section by default', () => {
    render(<SettingsScene {...defaultProps} />);
    // Both the nav button and the h2 say "Appearance"
    expect(screen.getAllByText('Appearance').length).toBeGreaterThan(0);
    // Palette grid is visible
    expect(screen.getByText('Sienna')).toBeInTheDocument();
  });

  it('switches to Sync section', () => {
    render(<SettingsScene {...defaultProps} />);
    fireEvent.click(screen.getAllByText('Sync')[0]);
    expect(screen.getByText('Sync status')).toBeInTheDocument();
  });

  it('switches to About section', () => {
    render(<SettingsScene {...defaultProps} />);
    fireEvent.click(screen.getByText('About'));
    expect(screen.getByText('License')).toBeInTheDocument();
  });

  it('calls onPalette when a palette swatch is clicked', () => {
    const onPalette = vi.fn();
    render(<SettingsScene {...defaultProps} onPalette={onPalette} />);
    fireEvent.click(screen.getByText('Slate'));
    expect(onPalette).toHaveBeenCalledWith('slate');
  });

  it('calls onBack when the back button is clicked', () => {
    const onBack = vi.fn();
    render(<SettingsScene {...defaultProps} onBack={onBack} />);
    fireEvent.click(screen.getByText('Back to files'));
    expect(onBack).toHaveBeenCalledOnce();
  });

  it('calls onPairs when Sync pairs nav item is clicked', () => {
    const onPairs = vi.fn();
    render(<SettingsScene {...defaultProps} onPairs={onPairs} />);
    fireEvent.click(screen.getByText('Sync pairs'));
    expect(onPairs).toHaveBeenCalledOnce();
  });

  it('shows Pause button when not paused', () => {
    render(<SettingsScene {...defaultProps} syncStatus={{ status: 'idle', active_file_count: 0, total_bytes: 0, transferred_bytes: 0, eta_seconds: null, last_sync_at: null }} />);
    fireEvent.click(screen.getAllByText('Sync')[0]);
    expect(screen.getByText('Pause')).toBeInTheDocument();
  });

  it('shows Resume button when paused', () => {
    render(<SettingsScene {...defaultProps} syncStatus={{ status: 'paused', active_file_count: 0, total_bytes: 0, transferred_bytes: 0, eta_seconds: null, last_sync_at: null }} />);
    fireEvent.click(screen.getAllByText('Sync')[0]);
    expect(screen.getByText('Resume')).toBeInTheDocument();
  });

  // T045 — "Stop background sync" button renders when daemon is running.
  it('renders stop background sync button when daemon running', async () => {
    const { findByText } = render(<SettingsScene {...defaultProps} />);
    fireEvent.click(screen.getAllByText('Sync')[0]);
    // The daemon section loads asynchronously (getDaemonStatus call)
    expect(await findByText(/stop background sync/i)).toBeInTheDocument();
  });

  // T046 — "Start background sync" button renders when daemon is stopped.
  it('renders start background sync button when daemon stopped', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_daemon_status') return Promise.resolve({ running: false, uptime_secs: null, connection_state: 'stopped' });
      if (cmd === 'get_status') return Promise.resolve({ status: 'idle', active_file_count: 0, total_bytes: 0, transferred_bytes: 0, eta_seconds: null, last_sync_at: null });
      if (cmd === 'get_bandwidth_status') return Promise.resolve({ upload_limit_kbps: 0, download_limit_kbps: 0, upload_rate_kbps: 0, download_rate_kbps: 0 });
      return Promise.resolve();
    });
    const { findAllByText } = render(<SettingsScene {...defaultProps} />);
    fireEvent.click(screen.getAllByText('Sync')[0]);
    // Use findAllByText since there may be multiple matches (the button + loading state)
    const buttons = await findAllByText(/start background sync/i);
    expect(buttons.length).toBeGreaterThan(0);
  });

  // T028 — Bandwidth section renders in the Sync tab.
  it('renders bandwidth section in Sync tab', async () => {
    render(<SettingsScene {...defaultProps} />);
    fireEvent.click(screen.getAllByText('Sync')[0]);
    await waitFor(() => {
      expect(screen.getByText('Bandwidth')).toBeInTheDocument();
    });
    expect(screen.getByTestId('upload-limit-input')).toBeInTheDocument();
    expect(screen.getByTestId('download-limit-input')).toBeInTheDocument();
  });

  // T029 — Save button calls setBandwidthLimits with parsed values.
  it('calls setBandwidthLimits on Save click', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    render(<SettingsScene {...defaultProps} />);
    fireEvent.click(screen.getAllByText('Sync')[0]);
    await waitFor(() => expect(screen.getByTestId('bandwidth-save-btn')).toBeInTheDocument());
    fireEvent.change(screen.getByTestId('upload-limit-input'), { target: { value: '500' } });
    fireEvent.change(screen.getByTestId('download-limit-input'), { target: { value: '1000' } });
    fireEvent.click(screen.getByTestId('bandwidth-save-btn'));
    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('set_bandwidth_limits', { uploadKbps: 500, downloadKbps: 1000 });
    });
  });

  // T030 — Clear button calls clearBandwidthLimits.
  it('calls clearBandwidthLimits on Clear click', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    render(<SettingsScene {...defaultProps} />);
    fireEvent.click(screen.getAllByText('Sync')[0]);
    await waitFor(() => expect(screen.getByTestId('bandwidth-clear-btn')).toBeInTheDocument());
    fireEvent.click(screen.getByTestId('bandwidth-clear-btn'));
    await waitFor(() => {
      expect(vi.mocked(invoke)).toHaveBeenCalledWith('clear_bandwidth_limits');
    });
  });

  // T031 — Live rate display shows after getBandwidthStatus resolves.
  it('shows live throughput rates', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    vi.mocked(invoke).mockImplementation((cmd: string) => {
      if (cmd === 'get_bandwidth_status') return Promise.resolve({ upload_limit_kbps: 500, download_limit_kbps: 0, upload_rate_kbps: 123, download_rate_kbps: 456 });
      if (cmd === 'get_daemon_status') return Promise.resolve({ running: true, uptime_secs: 60, connection_state: 'connected' });
      return Promise.resolve(null);
    });
    render(<SettingsScene {...defaultProps} />);
    fireEvent.click(screen.getAllByText('Sync')[0]);
    await waitFor(() => {
      expect(screen.getByTestId('live-upload-rate')).toHaveTextContent('123 Kbps');
      expect(screen.getByTestId('live-download-rate')).toHaveTextContent('456 Kbps');
    });
  });
});
