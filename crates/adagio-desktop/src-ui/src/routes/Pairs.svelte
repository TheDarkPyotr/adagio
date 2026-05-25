<script lang="ts">
  import { onMount } from "svelte";
  import { listAccounts, deletePair, listRemoteTree, type AccountDto, type RemoteTreeItemDto } from "../lib/tauri";
  import Onboarding from "./Onboarding.svelte";

  let accounts: AccountDto[] = [];
  let loaded = false;
  let showOnboarding = false;
  let selectedPairId: string | null = null;
  let remoteTree: RemoteTreeItemDto[] = [];
  let loadingTree = false;
  let selectiveFilter = "";

  onMount(async () => {
    try {
      accounts = await listAccounts();
    } catch {
      accounts = [];
    }
    loaded = true;
    if (accounts.length === 0) {
      showOnboarding = true;
    }
  });

  function handleOnboardingComplete() {
    showOnboarding = false;
    listAccounts().then((a) => (accounts = a));
  }

  async function openRemoteBrowser(pairId: string) {
    selectedPairId = pairId;
    loadingTree = true;
    try {
      remoteTree = await listRemoteTree(pairId);
    } catch {
      remoteTree = [];
    }
    loadingTree = false;
  }

  function closeRemoteBrowser() {
    selectedPairId = null;
    remoteTree = [];
    selectiveFilter = "";
  }

  $: filteredTree = selectiveFilter
    ? remoteTree.filter(i => i.path.toLowerCase().includes(selectiveFilter.toLowerCase()))
    : remoteTree;

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
</script>

{#if !loaded}
  <p>Loading…</p>
{:else if showOnboarding}
  <Onboarding />
{:else}
  <div class="pairs-view">
    <header>
      <h1>Sync Pairs</h1>
      <button on:click={() => (showOnboarding = true)}>+ Add account</button>
    </header>

    {#if accounts.length === 0}
      <p class="empty">No accounts configured. Click "+ Add account" to get started.</p>
    {:else}
      <ul>
        {#each accounts as account}
          <li>
            <span class="account-name">{account.display_name}</span>
            <span class="server">{account.server_url}</span>
            <button class="btn-sm" on:click={() => openRemoteBrowser(account.id)}>
              Selective Sync…
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </div>

  <!-- Selective sync tree browser -->
  {#if selectedPairId !== null}
    <div class="modal-overlay" role="dialog" aria-modal="true" aria-labelledby="tree-title">
      <div class="modal-tree">
        <div class="modal-header">
          <h3 id="tree-title">Selective Sync</h3>
          <button class="close-btn" on:click={closeRemoteBrowser} aria-label="Close">×</button>
        </div>

        <p class="modal-hint">Check the folders you want to sync to this device.</p>

        <input
          class="filter-input"
          type="search"
          placeholder="Filter paths…"
          bind:value={selectiveFilter}
        />

        {#if loadingTree}
          <p class="loading">Loading remote tree…</p>
        {:else if filteredTree.length === 0}
          <p class="empty-tree">No remote folders found.</p>
        {:else}
          <ul class="tree-list">
            {#each filteredTree as item (item.path)}
              <li class="tree-item" class:excluded={item.excluded} class:is-dir={item.is_dir}>
                <input type="checkbox" checked={!item.excluded} disabled />
                <span class="tree-icon">{item.is_dir ? "📁" : "📄"}</span>
                <span class="tree-path">{item.path}</span>
                {#if !item.is_dir}
                  <span class="tree-size">{formatSize(item.size)}</span>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}

        <div class="modal-footer">
          <button class="btn btn-primary" on:click={closeRemoteBrowser}>Done</button>
        </div>
      </div>
    </div>
  {/if}
{/if}

<style>
  .pairs-view {
    padding: 24px;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 24px;
  }

  h1 {
    font-size: 1.25rem;
  }

  ul {
    list-style: none;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  li {
    padding: 12px 16px;
    background: #252540;
    border-radius: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .account-name {
    font-weight: 600;
  }

  .server {
    font-size: 0.875rem;
    color: #888;
  }

  .empty {
    color: #888;
  }

  button {
    padding: 8px 16px;
    border: none;
    border-radius: 6px;
    background: #5865f2;
    color: #fff;
    cursor: pointer;
  }

  .btn-sm {
    padding: 4px 10px;
    font-size: 0.8rem;
    background: #374151;
    color: #f3f4f6;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    align-self: flex-start;
    margin-top: 4px;
  }

  /* Modal tree */
  .modal-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0,0,0,0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 200;
  }

  .modal-tree {
    background: white;
    border-radius: 10px;
    padding: 1.25rem;
    width: 420px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 8px 32px rgba(0,0,0,0.2);
  }

  .modal-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 0.5rem;
  }

  .modal-header h3 {
    font-size: 1rem;
    font-weight: 700;
    color: #111;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 1.25rem;
    cursor: pointer;
    color: #6b7280;
    padding: 0;
    line-height: 1;
  }

  .modal-hint {
    font-size: 0.8rem;
    color: #6b7280;
    margin-bottom: 0.75rem;
  }

  .filter-input {
    width: 100%;
    padding: 0.35rem 0.6rem;
    border: 1px solid #d1d5db;
    border-radius: 6px;
    font-size: 0.875rem;
    margin-bottom: 0.75rem;
    box-sizing: border-box;
  }

  .tree-list {
    list-style: none;
    padding: 0;
    margin: 0;
    overflow-y: auto;
    flex: 1;
  }

  .tree-item {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0.25rem 0;
    font-size: 0.85rem;
    color: #374151;
    border-bottom: 1px solid #f3f4f6;
  }

  .tree-item.excluded {
    opacity: 0.45;
  }

  .tree-icon {
    flex-shrink: 0;
  }

  .tree-path {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tree-size {
    font-size: 0.75rem;
    color: #9ca3af;
    flex-shrink: 0;
  }

  .loading, .empty-tree {
    color: #9ca3af;
    font-size: 0.875rem;
    text-align: center;
    padding: 1rem;
  }

  .modal-footer {
    display: flex;
    justify-content: flex-end;
    margin-top: 0.75rem;
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
</style>
