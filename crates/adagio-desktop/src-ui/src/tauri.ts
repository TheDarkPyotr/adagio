import { invoke } from '@tauri-apps/api/core';
import { listen, UnlistenFn } from '@tauri-apps/api/event';

// ── Account DTOs ──────────────────────────────────────────────────────────────

export interface AccountDto {
  id: string;
  display_name: string;
  server_url: string;
  username: string;
}

export interface AddAccountRequest {
  server_url: string;
  username: string;
  display_name: string;
  secret: string;
}

export const addAccount = (req: AddAccountRequest): Promise<AccountDto> =>
  invoke('add_account', { req });

export const removeAccount = (accountId: string): Promise<void> =>
  invoke('remove_account', { accountId });

export const listAccounts = (): Promise<AccountDto[]> =>
  invoke('list_accounts');

/** Run the complete OAuth2 browser-based account connection flow. */
export const connectAccountOAuth2 = (serverUrl: string): Promise<AccountDto> =>
  invoke('connect_account_oauth2', { serverUrl });

// ── Pair DTOs ─────────────────────────────────────────────────────────────────

export interface PairDto {
  id: string;
  account_id: string;
  local_root: string;
  remote_root: string;
  scan_interval_secs: number;
  selective_paths: string[];
}

export interface CreatePairRequest {
  account_id: string;
  local_root: string;
  remote_root: string;
}

export const createPair = (req: CreatePairRequest): Promise<PairDto> =>
  invoke('create_pair', { req });

export const deletePair = (pairId: string, deleteLocalFiles: boolean): Promise<void> =>
  invoke('delete_pair', { pairId, deleteLocalFiles });

export const listPairs = (): Promise<PairDto[]> =>
  invoke('list_pairs');

export const getExcludePatterns = (): Promise<string[]> =>
  invoke('get_exclude_patterns');

// ── Sync status ───────────────────────────────────────────────────────────────

export interface SyncStatusDto {
  status: 'idle' | 'syncing' | 'paused' | 'error';
  active_file_count: number;
  total_bytes: number;
  transferred_bytes: number;
  eta_seconds: number | null;
  last_sync_at: number | null;
}

export const getStatus = (): Promise<SyncStatusDto> =>
  invoke('get_status');

export const pauseSync = (): Promise<void> =>
  invoke('pause_sync');

export const resumeSync = (): Promise<void> =>
  invoke('resume_sync');

export const triggerSync = (pairId: string): Promise<void> =>
  invoke('trigger_sync', { pairId });

// ── Activity log ──────────────────────────────────────────────────────────────

export interface ActivityEntryDto {
  id: string;
  who: string;
  verb: string;
  target: string;
  with_whom: string | null;
  where_path: string;
  at: number;
  kind: 'edit' | 'share' | 'sync' | 'conflict';
}

export const getActivityLog = (
  limit?: number,
  filter?: 'edit' | 'share' | 'sync' | 'conflict',
): Promise<ActivityEntryDto[]> =>
  invoke('get_activity_log', { limit, filter });

// ── File browser ──────────────────────────────────────────────────────────────

export interface FileStatusDto {
  path: string;
  name: string;
  is_dir: boolean;
  size: number | null;
  mtime: number | null;
  status: 'ok' | 'sync' | 'cloud' | 'pin' | 'conflict';
  etag?: string;
  share_count?: number;
  item_count?: number;
}

export const listSyncedFiles = (
  pairId: string,
  relativePath?: string,
): Promise<FileStatusDto[]> =>
  invoke('list_synced_files', { pairId, relativePath });

// ── Error items ───────────────────────────────────────────────────────────────

export interface ErrorItemDto {
  path: string;
  message: string;
  retry_count: number;
}

export const getErrorItems = (pairId: string): Promise<ErrorItemDto[]> =>
  invoke('get_error_items', { pairId });

export interface RemoteTreeItemDto {
  path: string;
  is_dir: boolean;
  size: number;
  excluded: boolean;
}

export const listRemoteTree = (pairId: string): Promise<RemoteTreeItemDto[]> =>
  invoke('list_remote_tree', { pairId });

// ── Conflicts ─────────────────────────────────────────────────────────────────

export interface ConflictDto {
  id: string;
  pair_id: string;
  path: string;
  local_mtime: string;   // ISO-8601
  remote_mtime: string;  // ISO-8601
  local_size: number;    // bytes
  remote_size: number;   // bytes
  policy: string;
  resolution: string | null;
  detected_at: string;
  resolved_at: string | null;
  is_dir: boolean;
  conflict_kind: 'content_modified' | 'renamed_both_sides' | 'deleted_with_content';
}

export const listConflicts = (pairId: string): Promise<ConflictDto[]> =>
  invoke('list_conflicts', { pairId });

export const resolveConflict = (id: string, side: 'local' | 'remote' | 'both'): Promise<void> =>
  invoke('resolve_conflict', { id, side });

/** Dismiss all pending conflicts without any file I/O. Returns the number dismissed. */
export const dismissAllConflicts = (): Promise<number> =>
  invoke('dismiss_all_conflicts');

// ── Conflict push events ──────────────────────────────────────────────────────

export const listenConflictDetected = (
  cb: (payload: { pending_count: number }) => void,
): Promise<UnlistenFn> =>
  listen<{ pending_count: number }>('adagio://conflict-detected', (e) => cb(e.payload));

export const listenConflictResolved = (
  cb: (payload: { id: string; pending_count: number }) => void,
): Promise<UnlistenFn> =>
  listen<{ id: string; pending_count: number }>('adagio://conflict-resolved', (e) => cb(e.payload));

// ── Preferences ───────────────────────────────────────────────────────────────

/** Returns the persisted palette name, or "sienna" if none is set. */
export const getPalette = (): Promise<string> =>
  invoke('get_palette');

/** Persists the palette name (built-in or custom ID). */
export const setPalette = (name: string): Promise<void> =>
  invoke('set_palette', { name });

// ── Custom palettes ───────────────────────────────────────────────────────────

export interface CustomPaletteDto {
  id: string;
  name: string;
  cream: string;
  ink: string;
  accent: string;
}

export const listCustomPalettes = (): Promise<CustomPaletteDto[]> =>
  invoke('list_custom_palettes');

export const saveCustomPalette = (
  name: string,
  cream: string,
  ink: string,
  accent: string,
  id?: string,
): Promise<CustomPaletteDto> =>
  invoke('save_custom_palette', { id: id ?? null, name, cream, ink, accent });

export const deleteCustomPalette = (id: string): Promise<void> =>
  invoke('delete_custom_palette', { id });

// ── Sharing ───────────────────────────────────────────────────────────────────

export interface UserSearchResult {
  user_id: string;
  display_name: string;
}

export interface CreateShareRequest {
  account_id: string;
  path: string;
  recipients: Array<{ user_id: string; permission: number }>;
  expiry_date?: string;
  link_password?: string;
  hide_download: boolean;
  notify_on_open: boolean;
  note?: string;
}

export interface ShareResult {
  share_id: string;
  share_url: string;
}

export const searchUsers = (accountId: string, query: string): Promise<UserSearchResult[]> =>
  invoke('search_users', { accountId, query });

export const createShare = (request: CreateShareRequest): Promise<ShareResult> =>
  invoke('create_share', { request });

// ── Daemon lifecycle ──────────────────────────────────────────────────────────

export interface DaemonStatusDto {
  running: boolean;
  uptime_secs: number | null;
  connection_state: 'connected' | 'reconnecting' | 'stopped' | 'failed';
}

export const getDaemonStatus = (): Promise<DaemonStatusDto> =>
  invoke('get_daemon_status');

export const startDaemon = (): Promise<void> =>
  invoke('start_daemon');

export const stopDaemon = (): Promise<void> =>
  invoke('stop_daemon');

export const setStartAtLogin = (enabled: boolean): Promise<void> =>
  invoke('set_start_at_login', { enabled });

// ── Bandwidth throttling ──────────────────────────────────────────────────────

export interface BandwidthStatusDto {
  upload_limit_kbps: number;
  download_limit_kbps: number;
  upload_rate_kbps: number;
  download_rate_kbps: number;
}

export const getBandwidthStatus = (): Promise<BandwidthStatusDto> =>
  invoke('get_bandwidth_status');

export const setBandwidthLimits = (uploadKbps: number, downloadKbps: number): Promise<void> =>
  invoke('set_bandwidth_limits', { uploadKbps, downloadKbps });

export const clearBandwidthLimits = (): Promise<void> =>
  invoke('clear_bandwidth_limits');

// ── Network awareness ─────────────────────────────────────────────────────────

export type NetworkAction = 'allow' | 'throttle' | 'pause';

export interface NetworkPolicyDto {
  on_metered: NetworkAction;
  on_battery: NetworkAction;
  throttle_kbps: number;
  blocked_ssids: string[];
}

export interface NetworkStatusDto {
  metered: boolean;
  on_battery: boolean;
  ssid: string | null;
  effective_action: NetworkAction;
  throttle_kbps: number;
  reason: string;
  policy: NetworkPolicyDto;
}

export const getNetworkStatus = (): Promise<NetworkStatusDto> =>
  invoke('get_network_status');

export const setNetworkPolicy = (
  onMetered?: NetworkAction,
  onBattery?: NetworkAction,
  throttleKbps?: number,
): Promise<void> =>
  invoke('set_network_policy', {
    onMetered: onMetered ?? null,
    onBattery: onBattery ?? null,
    throttleKbps: throttleKbps ?? null,
  });

export const addBlockedSsid = (ssid: string): Promise<void> =>
  invoke('add_blocked_ssid', { ssid });

export const removeBlockedSsid = (ssid: string): Promise<void> =>
  invoke('remove_blocked_ssid', { ssid });

export const listBlockedSsids = (): Promise<string[]> =>
  invoke('list_blocked_ssids');

/** Listen for daemon connection state changes. */
export const listenDaemonConnectionState = (
  cb: (payload: { state: 'connected' | 'reconnecting' | 'stopped' | 'failed'; attempt?: number }) => void,
): Promise<UnlistenFn> =>
  listen<{ state: string; attempt?: number }>('adagio://daemon-connection-state', (e) =>
    cb(e.payload as { state: 'connected' | 'reconnecting' | 'stopped' | 'failed'; attempt?: number }),
  );
