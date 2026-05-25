import { invoke } from "@tauri-apps/api/core";

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

export interface PairDto {
  id: string;
  local_root: string;
  remote_root: string;
  account_id: string;
}

export interface CreatePairRequest {
  account_id: string;
  local_root: string;
  remote_root: string;
}

export const addAccount = (req: AddAccountRequest): Promise<AccountDto> =>
  invoke("add_account", { req });

export const removeAccount = (accountId: string): Promise<void> =>
  invoke("remove_account", { accountId });

export const listAccounts = (): Promise<AccountDto[]> =>
  invoke("list_accounts");

/** Run the complete OAuth2 browser-based account connection flow. */
export const connectAccountOAuth2 = (serverUrl: string): Promise<AccountDto> =>
  invoke("connect_account_oauth2", { serverUrl });

export const createPair = (req: CreatePairRequest): Promise<PairDto> =>
  invoke("create_pair", { req });

export const deletePair = (
  pairId: string,
  deleteLocalFiles: boolean
): Promise<void> => invoke("delete_pair", { pairId, deleteLocalFiles });

export const listPairs = (): Promise<PairDto[]> =>
  invoke("list_pairs");

export const getExcludePatterns = (): Promise<string[]> =>
  invoke("get_exclude_patterns");

// ── Sync control ──────────────────────────────────────────────────────────────

export interface SyncStatusDto {
  status: "idle" | "syncing" | "paused" | "error";
  pair_id?: string;
  error?: string;
}

export interface ActivityEntryDto {
  pair_id: string;
  path: string;
  action: string;
  occurred_at: string;
  bytes?: number;
}

export const getSyncStatus = (): Promise<SyncStatusDto> =>
  invoke("get_status");

export const pauseSync = (): Promise<void> =>
  invoke("pause_sync");

export const resumeSync = (): Promise<void> =>
  invoke("resume_sync");

export const getActivityLog = (limit?: number): Promise<ActivityEntryDto[]> =>
  invoke("get_activity_log", { limit });

export const triggerSync = (pairId: string): Promise<void> =>
  invoke("trigger_sync", { pairId });

export interface ErrorItemDto {
  path: string;
  error_message?: string;
  retry_count: number;
  updated_at: string;
}

export const getErrorItems = (pairId: string): Promise<ErrorItemDto[]> =>
  invoke("get_error_items", { pairId });

export interface RemoteTreeItemDto {
  path: string;
  is_dir: boolean;
  size: number;
  excluded: boolean;
}

export const listRemoteTree = (pairId: string): Promise<RemoteTreeItemDto[]> =>
  invoke("list_remote_tree", { pairId });

// ── File browser ──────────────────────────────────────────────────────────────

export interface FileStatusDto {
  name: string;
  relative_path: string;
  is_dir: boolean;
  size: number;
  modified_at: string;
  sync_status: string;
  error_message?: string;
}

export const listSyncedFiles = (
  pairId: string,
  relativePath?: string
): Promise<FileStatusDto[]> =>
  invoke("list_synced_files", { pairId, relativePath });

// ── Conflict management ───────────────────────────────────────────────────────

export interface ConflictDto {
  id: string;
  pair_id: string;
  path: string;
  local_mtime: string;
  remote_mtime: string;
  local_size: number;
  remote_size: number;
  policy: string;
  resolution?: string;
  detected_at: string;
  resolved_at?: string;
}

export const listConflicts = (pairId: string): Promise<ConflictDto[]> =>
  invoke("list_conflicts", { pairId });

export const resolveConflict = (
  id: string,
  side: "local" | "remote"
): Promise<void> => invoke("resolve_conflict", { id, side });
