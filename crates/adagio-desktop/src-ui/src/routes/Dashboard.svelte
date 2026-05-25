<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import {
    getSyncStatus,
    pauseSync,
    resumeSync,
    triggerSync,
    getActivityLog,
    getErrorItems,
    listPairs,
    listSyncedFiles,
    type SyncStatusDto,
    type ActivityEntryDto,
    type ErrorItemDto,
    type FileStatusDto,
    type PairDto,
  } from "../lib/tauri";

  let status: SyncStatusDto = { status: "idle" };
  let activityLog: ActivityEntryDto[] = [];
  let errorItems: ErrorItemDto[] = [];
  let error: string | null = null;
  let pollInterval: ReturnType<typeof setInterval> | null = null;
  let showReAuthModal = false;
  let currentPairId = "";

  // File browser state
  let pairs: PairDto[] = [];
  let selectedBrowserPairId: string = "";
  let currentSubPath: string = "";
  let fileBrowserEntries: FileStatusDto[] = [];
  let fileBrowserLoading = false;
  let fileBrowserError: string | null = null;
  let syncingPair = false;

  function breadcrumbs(): { label: string; path: string }[] {
    const crumbs = [{ label: "Root", path: "" }];
    if (!currentSubPath) return crumbs;
    const parts = currentSubPath.split("/");
    let accumulated = "";
    for (const part of parts) {
      accumulated = accumulated ? `${accumulated}/${part}` : part;
      crumbs.push({ label: part, path: accumulated });
    }
    return crumbs;
  }

  async function refresh() {
    try {
      [status, activityLog] = await Promise.all([
        getSyncStatus(),
        getActivityLog(50),
      ]);
      // If status shows a pair, load its error items.
      if (status.pair_id) {
        currentPairId = status.pair_id;
        errorItems = await getErrorItems(status.pair_id);
      }
      // Show re-auth modal when the engine signals auth required.
      if (status.status === "error" && status.error?.includes("auth required")) {
        showReAuthModal = true;
      }
      error = null;
    } catch (e) {
      error = String(e);
    }
    // Refresh file browser if a pair is selected.
    if (selectedBrowserPairId) {
      await loadFiles();
    }
  }

  async function loadFiles() {
    if (!selectedBrowserPairId) return;
    fileBrowserLoading = true;
    try {
      fileBrowserEntries = await listSyncedFiles(
        selectedBrowserPairId,
        currentSubPath || undefined
      );
      fileBrowserError = null;
    } catch (e) {
      fileBrowserError = String(e);
      fileBrowserEntries = [];
    } finally {
      fileBrowserLoading = false;
    }
  }

  async function handlePairSelect(pairId: string) {
    selectedBrowserPairId = pairId;
    currentSubPath = "";
    await loadFiles();
  }

  async function navigateTo(path: string) {
    currentSubPath = path;
    await loadFiles();
  }

  async function handleManualSync() {
    if (!selectedBrowserPairId) return;
    syncingPair = true;
    try {
      await triggerSync(selectedBrowserPairId);
      // Poll briefly to pick up updated statuses after the cycle completes.
      await new Promise((r) => setTimeout(r, 1500));
      await loadFiles();
    } finally {
      syncingPair = false;
    }
  }

  function dismissReAuth() {
    showReAuthModal = false;
  }

  async function handlePause() {
    await pauseSync();
    await refresh();
  }

  async function handleResume() {
    await resumeSync();
    await refresh();
  }

  async function handleTrigger(pairId: string) {
    await triggerSync(pairId);
    await refresh();
  }

  onMount(async () => {
    try {
      pairs = await listPairs();
      if (pairs.length > 0) {
        selectedBrowserPairId = pairs[0].id;
        await loadFiles();
      }
    } catch (e) {
      fileBrowserError = String(e);
      pairs = [];
    }
    refresh();
    pollInterval = setInterval(refresh, 5000);
  });

  onDestroy(() => {
    if (pollInterval) clearInterval(pollInterval);
  });

  function statusBadgeClass(s: string): string {
    switch (s) {
      case "syncing": return "badge badge-syncing";
      case "paused":  return "badge badge-paused";
      case "error":   return "badge badge-error";
      default:        return "badge badge-idle";
    }
  }

  function fileSyncBadgeClass(s: string): string {
    switch (s) {
      case "synced":           return "badge badge-synced";
      case "pending_upload":   return "badge badge-pending";
      case "pending_download": return "badge badge-pending";
      case "conflict":         return "badge badge-conflict";
      case "error":            return "badge badge-error";
      default:                 return "badge badge-unknown";
    }
  }

  function formatBytes(bytes?: number): string {
    if (!bytes) return "";
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function formatTime(iso: string): string {
    return new Date(iso).toLocaleTimeString();
  }

  function formatDate(iso: string): string {
    if (!iso) return "";
    return new Date(iso).toLocaleDateString(undefined, { month: "short", day: "numeric", hour: "2-digit", minute: "2-digit" });
  }

</script>

<div class="dashboard">
  <!-- Re-auth modal -->
  {#if showReAuthModal}
    <div class="modal-overlay" role="dialog" aria-modal="true" aria-labelledby="reauth-title">
      <div class="modal">
        <h3 id="reauth-title">Re-authentication Required</h3>
        <p>Your session has expired or credentials are no longer valid. Please update your account credentials to resume syncing.</p>
        <div class="modal-actions">
          <button class="btn btn-primary" on:click={dismissReAuth}>
            Go to Accounts
          </button>
          <button class="btn btn-secondary" on:click={dismissReAuth}>
            Dismiss
          </button>
        </div>
      </div>
    </div>
  {/if}

  <!-- Status header -->
  <section class="status-section">
    <h2>Sync Status</h2>

    {#if error}
      <p class="error-message">{error}</p>
    {:else}
      <div class="status-row">
        <span class={statusBadgeClass(status.status)}>
          {status.status.toUpperCase()}
        </span>
        {#if status.pair_id}
          <span class="pair-id">Pair: {status.pair_id}</span>
        {/if}
        {#if status.error}
          <span class="status-error">{status.error}</span>
        {/if}
      </div>

      <div class="controls">
        {#if status.status === "paused"}
          <button class="btn btn-primary" on:click={handleResume}>
            Resume Sync
          </button>
        {:else if status.status !== "error"}
          <button class="btn btn-secondary" on:click={handlePause}>
            Pause Sync
          </button>
        {/if}
      </div>
    {/if}
  </section>

  <!-- Error items (parked transfers) -->
  {#if errorItems.length > 0}
    <section class="error-section">
      <h2>Transfer Errors ({errorItems.length})</h2>
      <ul class="error-list">
        {#each errorItems as item (item.path + item.updated_at)}
          <li class="error-item">
            <span class="error-path">{item.path}</span>
            {#if item.error_message}
              <span class="error-cause">{item.error_message}</span>
            {/if}
            <span class="error-retries">Retried {item.retry_count}×</span>
          </li>
        {/each}
      </ul>
    </section>
  {/if}

  <!-- Activity log feed -->
  <section class="activity-section">
    <h2>Activity Log</h2>

    {#if activityLog.length === 0}
      <p class="empty">No activity yet.</p>
    {:else}
      <ul class="activity-list">
        {#each activityLog as entry (entry.occurred_at + entry.path)}
          <li class="activity-item">
            <span class="activity-time">{formatTime(entry.occurred_at)}</span>
            <span class="activity-action">{entry.action}</span>
            <span class="activity-path">{entry.path}</span>
            {#if entry.bytes}
              <span class="activity-bytes">{formatBytes(entry.bytes)}</span>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}
  </section>

  <!-- File browser panel -->
  <section class="browser-section">
    <div class="browser-header">
      <h2>File Browser</h2>
      <div class="browser-controls">
        {#if pairs.length > 0}
          <select
            class="pair-select"
            bind:value={selectedBrowserPairId}
            on:change={() => handlePairSelect(selectedBrowserPairId)}
          >
            <option value="">Select a pair…</option>
            {#each pairs as pair (pair.id)}
              <option value={pair.id}>{pair.local_root}</option>
            {/each}
          </select>
        {:else}
          <span class="empty">No pairs configured.</span>
        {/if}
        {#if selectedBrowserPairId}
          <button
            class="btn btn-primary"
            on:click={handleManualSync}
            disabled={syncingPair}
          >
            {syncingPair ? "Syncing…" : "Sync Now"}
          </button>
        {/if}
      </div>
    </div>

    {#if selectedBrowserPairId}
      <!-- Breadcrumb navigation -->
      <nav class="breadcrumb">
        {#each breadcrumbs() as crumb, i}
          {#if i > 0}<span class="breadcrumb-sep">›</span>{/if}
          {#if i < breadcrumbs().length - 1}
            <button class="breadcrumb-link" on:click={() => navigateTo(crumb.path)}>
              {crumb.label}
            </button>
          {:else}
            <span class="breadcrumb-current">{crumb.label}</span>
          {/if}
        {/each}
      </nav>
    {/if}

    {#if fileBrowserError}
      <p class="error-message">{fileBrowserError}</p>
    {:else if fileBrowserLoading}
      <p class="empty">Loading…</p>
    {:else if selectedBrowserPairId && fileBrowserEntries.length === 0}
      <p class="empty">No local files in this folder. Sync to download from Nextcloud.</p>
    {:else if selectedBrowserPairId}
      <table class="file-table">
        <thead>
          <tr>
            <th class="col-name">Name</th>
            <th class="col-size">Size</th>
            <th class="col-date">Modified</th>
            <th class="col-status">Status</th>
          </tr>
        </thead>
        <tbody>
          {#each fileBrowserEntries as entry (entry.relative_path)}
            <tr
              class:row-dir={entry.is_dir}
              on:click={() => entry.is_dir && navigateTo(entry.relative_path)}
            >
              <td class="col-name">
                <span class="file-icon">{entry.is_dir ? "📁" : "📄"}</span>
                <span class="file-name">{entry.name}</span>
              </td>
              <td class="col-size">{entry.is_dir ? "" : formatBytes(entry.size)}</td>
              <td class="col-date">{formatDate(entry.modified_at)}</td>
              <td class="col-status">
                <span class={fileSyncBadgeClass(entry.sync_status)}>
                  {entry.sync_status}
                </span>
                {#if entry.error_message}
                  <span class="file-error" title={entry.error_message}>⚠</span>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {:else}
      <p class="empty">Select a pair above to browse files.</p>
    {/if}
  </section>
</div>

<style>
  .dashboard {
    padding: 1.5rem;
    font-family: system-ui, sans-serif;
    max-width: 800px;
  }

  h2 {
    font-size: 1.1rem;
    font-weight: 600;
    margin-bottom: 0.75rem;
    color: #111;
  }

  .status-section,
  .activity-section {
    margin-bottom: 2rem;
    padding: 1rem;
    background: #f9f9f9;
    border-radius: 8px;
    border: 1px solid #e5e5e5;
  }

  .status-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    margin-bottom: 0.75rem;
  }

  .badge {
    display: inline-block;
    padding: 0.2rem 0.6rem;
    border-radius: 999px;
    font-size: 0.75rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .badge-idle    { background: #e5e7eb; color: #374151; }
  .badge-syncing { background: #dbeafe; color: #1d4ed8; }
  .badge-paused  { background: #fef9c3; color: #92400e; }
  .badge-error   { background: #fee2e2; color: #991b1b; }

  .controls {
    display: flex;
    gap: 0.5rem;
  }

  .btn {
    padding: 0.35rem 0.9rem;
    border-radius: 6px;
    font-size: 0.85rem;
    font-weight: 500;
    cursor: pointer;
    border: 1px solid transparent;
  }

  .btn-primary {
    background: #2563eb;
    color: white;
  }

  .btn-secondary {
    background: white;
    color: #374151;
    border-color: #d1d5db;
  }

  .error-message {
    color: #dc2626;
    font-size: 0.875rem;
  }

  .pair-id, .status-error {
    font-size: 0.8rem;
    color: #6b7280;
  }

  .empty {
    color: #9ca3af;
    font-size: 0.875rem;
  }

  .activity-list {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .activity-item {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    padding: 0.35rem 0;
    border-bottom: 1px solid #f0f0f0;
    font-size: 0.85rem;
  }

  .activity-time {
    color: #9ca3af;
    width: 5rem;
    flex-shrink: 0;
  }

  .activity-action {
    color: #2563eb;
    font-weight: 500;
    width: 6rem;
    flex-shrink: 0;
  }

  .activity-path {
    color: #374151;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .activity-bytes {
    color: #6b7280;
    flex-shrink: 0;
  }

  /* Modal */
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal {
    background: white;
    border-radius: 10px;
    padding: 1.5rem;
    max-width: 380px;
    width: 90%;
    box-shadow: 0 8px 32px rgba(0,0,0,0.18);
  }

  .modal h3 {
    font-size: 1rem;
    font-weight: 700;
    margin-bottom: 0.75rem;
    color: #111;
  }

  .modal p {
    font-size: 0.875rem;
    color: #374151;
    margin-bottom: 1.25rem;
    line-height: 1.5;
  }

  .modal-actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
  }

  /* File browser */
  .browser-section {
    margin-bottom: 2rem;
    padding: 1rem;
    background: #f9f9f9;
    border-radius: 8px;
    border: 1px solid #e5e5e5;
  }

  .browser-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.75rem;
    flex-wrap: wrap;
    gap: 0.5rem;
  }

  .browser-header h2 {
    margin-bottom: 0;
  }

  .browser-controls {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .pair-select {
    padding: 0.3rem 0.6rem;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 0.85rem;
    background: white;
    color: #374151;
    max-width: 280px;
  }

  .breadcrumb {
    display: flex;
    align-items: center;
    gap: 0.25rem;
    margin-bottom: 0.5rem;
    font-size: 0.8rem;
  }

  .breadcrumb-sep {
    color: #9ca3af;
  }

  .breadcrumb-link {
    background: none;
    border: none;
    padding: 0;
    color: #2563eb;
    cursor: pointer;
    font-size: 0.8rem;
    text-decoration: underline;
  }

  .breadcrumb-current {
    color: #374151;
    font-weight: 500;
  }

  .row-dir {
    cursor: pointer;
  }

  .row-dir:hover {
    background: #f0f4ff;
  }

  .file-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }

  .file-table th {
    text-align: left;
    font-weight: 600;
    color: #6b7280;
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 0.4rem 0.5rem;
    border-bottom: 1px solid #e5e5e5;
  }

  .file-table td {
    padding: 0.4rem 0.5rem;
    border-bottom: 1px solid #f0f0f0;
    color: #374151;
    vertical-align: middle;
  }

  .file-table tr:last-child td {
    border-bottom: none;
  }

  .col-name { width: 45%; }
  .col-size { width: 12%; text-align: right; color: #9ca3af; }
  .col-date { width: 23%; color: #9ca3af; }
  .col-status { width: 20%; }

  .file-icon {
    margin-right: 0.3rem;
  }

  .file-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .file-error {
    margin-left: 0.25rem;
    color: #dc2626;
    cursor: help;
  }

  .badge-synced    { background: #d1fae5; color: #065f46; }
  .badge-pending   { background: #dbeafe; color: #1d4ed8; }
  .badge-conflict  { background: #fef3c7; color: #92400e; }
  .badge-unknown   { background: #f3f4f6; color: #6b7280; }

  /* Error items section */
  .error-section {
    margin-bottom: 2rem;
    padding: 1rem;
    background: #fff5f5;
    border-radius: 8px;
    border: 1px solid #fecaca;
  }

  .error-section h2 {
    color: #991b1b;
  }

  .error-list {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  .error-item {
    display: flex;
    align-items: baseline;
    gap: 0.5rem;
    padding: 0.35rem 0;
    border-bottom: 1px solid #fecaca;
    font-size: 0.85rem;
  }

  .error-path {
    color: #374151;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
  }

  .error-cause {
    color: #dc2626;
    font-size: 0.8rem;
    flex-shrink: 0;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .error-retries {
    color: #9ca3af;
    font-size: 0.75rem;
    flex-shrink: 0;
  }
</style>
