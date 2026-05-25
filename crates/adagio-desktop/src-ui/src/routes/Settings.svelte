<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import {
    listAccounts,
    connectAccountOAuth2,
    removeAccount,
    type AccountDto,
  } from "../lib/tauri";

  interface BandwidthSettings {
    upload_kbps: number;   // 0 = unlimited
    download_kbps: number; // 0 = unlimited
    max_upload_concurrency: number;
    max_download_concurrency: number;
    pause_on_metered: boolean;
    min_battery_percent: number; // 0 = disabled
  }

  interface TimeWindowDto {
    start_hour: number;
    end_hour: number;
    cap_kbps: number;
  }

  let settings: BandwidthSettings = {
    upload_kbps: 0,
    download_kbps: 0,
    max_upload_concurrency: 3,
    max_download_concurrency: 3,
    pause_on_metered: false,
    min_battery_percent: 0,
  };

  let schedule: TimeWindowDto[] = [];
  let saving = false;
  let saved = false;
  let error = "";

  // New schedule row form state
  let newStartHour = 9;
  let newEndHour = 17;
  let newCapKbps = 512;

  // ── Accounts state ─────────────────────────────────────────────────────────
  let accounts: AccountDto[] = [];
  let accountsError = "";
  let addingAccount = false;
  let addAccountServerUrl = "";
  let showAddAccount = false;

  // Per-account removal confirmation state
  let confirmRemoveId: string | null = null;

  onMount(async () => {
    try {
      settings = await invoke<BandwidthSettings>("get_bandwidth_settings");
      schedule = await invoke<TimeWindowDto[]>("get_bandwidth_schedule");
    } catch {
      // Settings not yet persisted — use defaults silently
    }
    try {
      accounts = await listAccounts();
    } catch (e) {
      accountsError = String(e);
    }
  });

  async function handleAddAccount() {
    if (!addAccountServerUrl.trim()) return;
    accountsError = "";
    addingAccount = true;
    try {
      const newAccount = await connectAccountOAuth2(addAccountServerUrl.trim());
      accounts = [...accounts, newAccount];
      addAccountServerUrl = "";
      showAddAccount = false;
    } catch (e) {
      accountsError = String(e);
    } finally {
      addingAccount = false;
    }
  }

  async function handleRemoveAccount(id: string) {
    accountsError = "";
    try {
      await removeAccount(id);
      accounts = accounts.filter((a) => a.id !== id);
      confirmRemoveId = null;
    } catch (e) {
      accountsError = String(e);
      confirmRemoveId = null;
    }
  }

  async function save() {
    saving = true;
    error = "";
    try {
      await invoke("set_bandwidth_settings", { settings });
      await invoke("set_bandwidth_schedule", { schedule });
      saved = true;
      setTimeout(() => (saved = false), 2000);
    } catch (e) {
      error = String(e);
    } finally {
      saving = false;
    }
  }

  function addWindow() {
    if (newStartHour >= newEndHour) {
      error = "Start hour must be before end hour.";
      return;
    }
    error = "";
    schedule = [...schedule, { start_hour: newStartHour, end_hour: newEndHour, cap_kbps: newCapKbps }];
  }

  function removeWindow(i: number) {
    schedule = schedule.filter((_, idx) => idx !== i);
  }

  function formatHour(h: number): string {
    const ampm = h < 12 ? "AM" : "PM";
    const display = h % 12 === 0 ? 12 : h % 12;
    return `${display}:00 ${ampm}`;
  }
</script>

<div class="settings">
  <!-- ── Accounts section ───────────────────────────────────────────────── -->
  <h2>Accounts</h2>

  {#if accountsError}
    <p class="error">{accountsError}</p>
  {/if}

  <section>
    <h3>Connected Accounts</h3>

    {#if accounts.length === 0}
      <p class="empty">No accounts connected yet.</p>
    {:else}
      <ul class="account-list">
        {#each accounts as account (account.id)}
          <li class="account-row">
            <div class="account-info">
              <span class="account-name">{account.display_name}</span>
              <span class="account-server">{account.server_url}</span>
            </div>
            {#if confirmRemoveId === account.id}
              <div class="confirm-remove">
                <span>Remove <strong>{account.display_name}</strong>? This also deletes all sync pairs.</span>
                <button class="danger-btn" on:click={() => handleRemoveAccount(account.id)}>Confirm</button>
                <button class="cancel-btn" on:click={() => (confirmRemoveId = null)}>Cancel</button>
              </div>
            {:else}
              <button class="remove-btn" on:click={() => (confirmRemoveId = account.id)} aria-label="Remove account">
                Remove
              </button>
            {/if}
          </li>
        {/each}
      </ul>
    {/if}

    {#if showAddAccount}
      <form class="add-account-form" on:submit|preventDefault={handleAddAccount}>
        <label>
          Nextcloud server URL
          <input
            type="url"
            bind:value={addAccountServerUrl}
            placeholder="https://cloud.example.com"
            required
            disabled={addingAccount}
          />
        </label>
        {#if addingAccount}
          <div class="waiting-banner">
            <span aria-hidden="true">⏳</span>
            Waiting for browser — complete sign-in then return here.
          </div>
        {/if}
        <div class="form-actions">
          <button type="button" on:click={() => { showAddAccount = false; addAccountServerUrl = ""; accountsError = ""; }} disabled={addingAccount}>
            Cancel
          </button>
          <button type="submit" class="primary-btn" disabled={addingAccount}>
            {addingAccount ? "Connecting…" : "Sign in with browser"}
          </button>
        </div>
      </form>
    {:else}
      <button class="add-btn" on:click={() => (showAddAccount = true)}>
        + Add Account
      </button>
    {/if}
  </section>

  <!-- ── Bandwidth section ─────────────────────────────────────────────── -->
  <h2>Bandwidth & Transfer Settings</h2>

  {#if error}
    <p class="error">{error}</p>
  {/if}

  <section>
    <h3>Speed Limits</h3>
    <p class="hint">Set to 0 for unlimited.</p>
    <div class="field-row">
      <label for="upload-kbps">Upload limit (KB/s)</label>
      <input id="upload-kbps" type="number" min="0" bind:value={settings.upload_kbps} />
    </div>
    <div class="field-row">
      <label for="download-kbps">Download limit (KB/s)</label>
      <input id="download-kbps" type="number" min="0" bind:value={settings.download_kbps} />
    </div>
  </section>

  <section>
    <h3>Concurrent Transfers</h3>
    <div class="field-row">
      <label for="upload-concurrency">Max simultaneous uploads</label>
      <input id="upload-concurrency" type="number" min="1" max="10" bind:value={settings.max_upload_concurrency} />
    </div>
    <div class="field-row">
      <label for="download-concurrency">Max simultaneous downloads</label>
      <input id="download-concurrency" type="number" min="1" max="10" bind:value={settings.max_download_concurrency} />
    </div>
  </section>

  <section>
    <h3>Smart Sync</h3>
    <div class="field-row checkbox-row">
      <input id="pause-metered" type="checkbox" bind:checked={settings.pause_on_metered} />
      <label for="pause-metered">Pause sync on metered connections</label>
    </div>
    <div class="field-row">
      <label for="min-battery">Pause sync below battery % (0 = disabled)</label>
      <input id="min-battery" type="number" min="0" max="100" bind:value={settings.min_battery_percent} />
    </div>
  </section>

  <section>
    <h3>Bandwidth Schedule</h3>
    <p class="hint">Override speed limits during specific hours. Windows are applied in order; first match wins.</p>

    {#if schedule.length === 0}
      <p class="empty">No scheduled windows. Limits above apply at all times.</p>
    {:else}
      <table>
        <thead>
          <tr><th>Start</th><th>End</th><th>Cap (KB/s)</th><th></th></tr>
        </thead>
        <tbody>
          {#each schedule as win, i}
            <tr>
              <td>{formatHour(win.start_hour)}</td>
              <td>{formatHour(win.end_hour)}</td>
              <td>{win.cap_kbps === 0 ? "Unlimited" : win.cap_kbps}</td>
              <td>
                <button class="remove-btn" on:click={() => removeWindow(i)} aria-label="Remove window">✕</button>
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}

    <div class="add-window">
      <select bind:value={newStartHour}>
        {#each Array.from({ length: 24 }, (_, i) => i) as h}
          <option value={h}>{formatHour(h)}</option>
        {/each}
      </select>
      <span>to</span>
      <select bind:value={newEndHour}>
        {#each Array.from({ length: 24 }, (_, i) => i) as h}
          <option value={h}>{formatHour(h)}</option>
        {/each}
      </select>
      <input type="number" min="0" placeholder="KB/s (0=unlimited)" bind:value={newCapKbps} style="width:120px" />
      <button class="add-btn" on:click={addWindow}>Add Window</button>
    </div>
  </section>

  <div class="actions">
    <button class="save-btn" on:click={save} disabled={saving}>
      {saving ? "Saving…" : saved ? "Saved!" : "Save Settings"}
    </button>
  </div>
</div>

<style>
  .settings {
    max-width: 640px;
    margin: 1.5rem auto;
    padding: 0 1rem;
    color: #e0e0e0;
  }

  h2 {
    font-size: 1.2rem;
    margin-bottom: 1.5rem;
    color: #ccc;
  }

  section {
    background: #16213e;
    border-radius: 8px;
    padding: 1rem 1.25rem;
    margin-bottom: 1rem;
  }

  h3 {
    font-size: 0.9rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #888;
    margin-bottom: 0.75rem;
  }

  .hint {
    font-size: 0.8rem;
    color: #666;
    margin-bottom: 0.75rem;
  }

  .field-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.6rem;
    gap: 1rem;
  }

  .checkbox-row {
    justify-content: flex-start;
    gap: 0.6rem;
  }

  label {
    font-size: 0.875rem;
    flex: 1;
  }

  input[type="number"] {
    width: 80px;
    background: #0f3460;
    border: 1px solid #444;
    border-radius: 4px;
    padding: 0.3rem 0.5rem;
    color: #e0e0e0;
    font-size: 0.875rem;
    text-align: right;
  }

  input[type="checkbox"] {
    width: 16px;
    height: 16px;
    cursor: pointer;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    margin-bottom: 0.75rem;
    font-size: 0.85rem;
  }

  th, td {
    text-align: left;
    padding: 0.4rem 0.5rem;
    border-bottom: 1px solid #2a2a4a;
  }

  th {
    color: #888;
    font-weight: 500;
  }

  .remove-btn {
    background: none;
    border: none;
    color: #888;
    cursor: pointer;
    padding: 0.1rem 0.3rem;
    font-size: 0.8rem;
  }

  .remove-btn:hover {
    color: #e74c3c;
  }

  .empty {
    font-size: 0.85rem;
    color: #666;
    margin-bottom: 0.75rem;
  }

  .add-window {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex-wrap: wrap;
    font-size: 0.85rem;
  }

  select {
    background: #0f3460;
    border: 1px solid #444;
    border-radius: 4px;
    padding: 0.3rem 0.4rem;
    color: #e0e0e0;
    font-size: 0.85rem;
  }

  .add-btn {
    background: #16213e;
    border: 1px solid #444;
    border-radius: 4px;
    padding: 0.3rem 0.75rem;
    color: #e0e0e0;
    cursor: pointer;
    font-size: 0.85rem;
  }

  .add-btn:hover {
    background: #1a285e;
  }

  .actions {
    display: flex;
    justify-content: flex-end;
    margin-top: 0.5rem;
  }

  .save-btn {
    background: #3498db;
    border: none;
    border-radius: 6px;
    padding: 0.6rem 1.5rem;
    color: #fff;
    font-size: 0.9rem;
    font-weight: 600;
    cursor: pointer;
  }

  .save-btn:hover:not(:disabled) {
    background: #2980b9;
  }

  .save-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .error {
    color: #e74c3c;
    font-size: 0.85rem;
    background: rgba(231, 76, 60, 0.1);
    padding: 0.5rem 0.75rem;
    border-radius: 4px;
    margin-bottom: 1rem;
  }

  /* ── Accounts ────────────────────────────────────────────────────────── */
  .account-list {
    list-style: none;
    margin-bottom: 0.75rem;
  }

  .account-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.6rem 0;
    border-bottom: 1px solid #2a2a4a;
    gap: 1rem;
    flex-wrap: wrap;
  }

  .account-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .account-name {
    font-size: 0.9rem;
    color: #e0e0e0;
    font-weight: 500;
  }

  .account-server {
    font-size: 0.78rem;
    color: #888;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .confirm-remove {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.82rem;
    color: #f87171;
    flex-wrap: wrap;
  }

  .danger-btn {
    background: #7f1d1d;
    border: 1px solid #dc2626;
    border-radius: 4px;
    padding: 0.25rem 0.6rem;
    color: #fca5a5;
    cursor: pointer;
    font-size: 0.8rem;
  }

  .danger-btn:hover {
    background: #991b1b;
  }

  .cancel-btn {
    background: #16213e;
    border: 1px solid #444;
    border-radius: 4px;
    padding: 0.25rem 0.6rem;
    color: #e0e0e0;
    cursor: pointer;
    font-size: 0.8rem;
  }

  .add-account-form {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    margin-top: 0.75rem;
  }

  .add-account-form label {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 0.85rem;
    color: #aaa;
  }

  .add-account-form input[type="url"] {
    width: 100%;
    padding: 0.4rem 0.6rem;
    background: #252540;
    border: 1px solid #444;
    border-radius: 4px;
    color: #e0e0e0;
    font-size: 0.9rem;
  }

  .waiting-banner {
    background: #1e2a4a;
    border: 1px solid #3a4a7a;
    border-radius: 6px;
    padding: 10px 14px;
    font-size: 0.85rem;
    color: #a0b4e0;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .form-actions {
    display: flex;
    gap: 0.5rem;
    justify-content: flex-end;
  }

  .primary-btn {
    background: #5865f2;
    border: none;
    border-radius: 4px;
    padding: 0.35rem 0.9rem;
    color: #fff;
    cursor: pointer;
    font-size: 0.85rem;
  }

  .primary-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }
</style>
