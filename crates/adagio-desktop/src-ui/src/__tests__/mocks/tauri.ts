// Stub the Tauri invoke bridge so components render without a Tauri runtime.
// Using vi.fn(defaultImpl) means vi.resetAllMocks() restores the default behaviour.
const defaultInvoke = (cmd: string): Promise<unknown> => {
  switch (cmd) {
    case 'get_activity_log': return Promise.resolve([]);
    case 'list_pairs': return Promise.resolve([]);
    case 'list_accounts': return Promise.resolve([]);
    case 'list_synced_files': return Promise.resolve([]);
    case 'get_status': return Promise.resolve({
      status: 'idle',
      active_file_count: 0,
      total_bytes: 0,
      transferred_bytes: 0,
      eta_seconds: null,
      last_sync_at: null,
    });
    case 'get_daemon_status': return Promise.resolve({
      running: true,
      uptime_secs: 120,
      connection_state: 'connected',
    });
    case 'start_daemon': return Promise.resolve();
    case 'stop_daemon': return Promise.resolve();
    case 'set_start_at_login': return Promise.resolve();
    case 'get_bandwidth_status': return Promise.resolve({
      upload_limit_kbps: 0,
      download_limit_kbps: 0,
      upload_rate_kbps: 0,
      download_rate_kbps: 0,
    });
    case 'set_bandwidth_limits': return Promise.resolve();
    case 'clear_bandwidth_limits': return Promise.resolve();
    default: return Promise.resolve(null);
  }
};

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(defaultInvoke),
}));

vi.mock('@tauri-apps/plugin-shell', () => ({
  open: vi.fn().mockResolvedValue(undefined),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock('@tauri-apps/api/webviewWindow', () => ({
  getCurrentWebviewWindow: () => ({
    startDragging: vi.fn().mockResolvedValue(undefined),
    minimize: vi.fn().mockResolvedValue(undefined),
    toggleMaximize: vi.fn().mockResolvedValue(undefined),
    close: vi.fn().mockResolvedValue(undefined),
    show: vi.fn().mockResolvedValue(undefined),
    setFocus: vi.fn().mockResolvedValue(undefined),
  }),
  getAllWebviewWindows: vi.fn().mockResolvedValue([]),
}));
