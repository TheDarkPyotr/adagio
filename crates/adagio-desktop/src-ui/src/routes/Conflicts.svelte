<script lang="ts">
  import { onMount } from "svelte";
  import { listConflicts, resolveConflict, type ConflictDto } from "../lib/tauri";

  export let pairId: string = "";

  let conflicts: ConflictDto[] = [];
  let loading = true;
  let error = "";

  async function load() {
    if (!pairId) return;
    loading = true;
    error = "";
    try {
      conflicts = await listConflicts(pairId);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function resolve(id: string, side: "local" | "remote") {
    try {
      await resolveConflict(id, side);
      await load();
    } catch (e) {
      error = String(e);
    }
  }

  function formatDate(iso: string): string {
    return new Date(iso).toLocaleString();
  }

  function formatBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  }

  onMount(load);
</script>

<div class="conflicts">
  <h2>Conflicts</h2>

  {#if !pairId}
    <p class="hint">Select a sync pair to view its conflicts.</p>
  {:else if loading}
    <p class="hint">Loading…</p>
  {:else if error}
    <p class="error">{error}</p>
  {:else if conflicts.length === 0}
    <p class="hint">No conflicts for this pair.</p>
  {:else}
    <table>
      <thead>
        <tr>
          <th>Path</th>
          <th>Local</th>
          <th>Remote</th>
          <th>Policy</th>
          <th>Detected</th>
          <th>Status</th>
          <th>Actions</th>
        </tr>
      </thead>
      <tbody>
        {#each conflicts as c (c.id)}
          <tr class={c.resolution ? "resolved" : "unresolved"}>
            <td class="path">{c.path}</td>
            <td>
              <span class="mtime">{formatDate(c.local_mtime)}</span><br />
              <span class="size">{formatBytes(c.local_size)}</span>
            </td>
            <td>
              <span class="mtime">{formatDate(c.remote_mtime)}</span><br />
              <span class="size">{formatBytes(c.remote_size)}</span>
            </td>
            <td><code>{c.policy}</code></td>
            <td>{formatDate(c.detected_at)}</td>
            <td>
              {#if c.resolution}
                <span class="badge resolved">{c.resolution}</span>
              {:else}
                <span class="badge pending">Pending</span>
              {/if}
            </td>
            <td class="actions">
              {#if !c.resolution}
                <button on:click={() => resolve(c.id, "local")}>Keep Local</button>
                <button on:click={() => resolve(c.id, "remote")}>Keep Remote</button>
              {:else}
                <span class="resolved-at">
                  {c.resolved_at ? formatDate(c.resolved_at) : ""}
                </span>
              {/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  {/if}
</div>

<style>
  .conflicts {
    padding: 1rem;
    font-family: system-ui, sans-serif;
  }

  h2 {
    margin-bottom: 1rem;
    font-size: 1.2rem;
  }

  .hint {
    color: #888;
    font-style: italic;
  }

  .error {
    color: #c0392b;
    background: #fdecea;
    padding: 0.5rem;
    border-radius: 4px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.85rem;
  }

  th, td {
    text-align: left;
    padding: 0.4rem 0.6rem;
    border-bottom: 1px solid #e0e0e0;
  }

  th {
    font-weight: 600;
    background: #f5f5f5;
  }

  .path {
    font-family: monospace;
    max-width: 200px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mtime {
    font-size: 0.78rem;
  }

  .size {
    color: #666;
    font-size: 0.75rem;
  }

  .badge {
    display: inline-block;
    padding: 0.2rem 0.5rem;
    border-radius: 12px;
    font-size: 0.75rem;
    font-weight: 600;
  }

  .badge.pending {
    background: #fff3cd;
    color: #856404;
  }

  .badge.resolved {
    background: #d4edda;
    color: #155724;
  }

  tr.resolved td {
    opacity: 0.7;
  }

  .actions {
    white-space: nowrap;
  }

  .actions button {
    margin-right: 0.3rem;
    padding: 0.2rem 0.6rem;
    font-size: 0.8rem;
    border: 1px solid #ccc;
    border-radius: 4px;
    cursor: pointer;
    background: #fff;
  }

  .actions button:hover {
    background: #f0f0f0;
  }

  .resolved-at {
    color: #888;
    font-size: 0.75rem;
  }
</style>
