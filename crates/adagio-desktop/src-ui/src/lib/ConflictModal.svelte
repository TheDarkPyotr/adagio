<script lang="ts">
  import { createEventDispatcher } from "svelte";
  import { resolveConflict, type ConflictDto } from "./tauri";

  export let conflict: ConflictDto | null = null;

  const dispatch = createEventDispatcher<{
    resolved: void;
    dismissed: void;
  }>();

  let resolving = false;
  let error = "";

  function formatDate(iso: string): string {
    return new Date(iso).toLocaleString();
  }

  function formatBytes(n: number): string {
    if (n < 1024) return `${n} B`;
    if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
    return `${(n / (1024 * 1024)).toFixed(1)} MB`;
  }

  async function choose(side: "local" | "remote") {
    if (!conflict || resolving) return;
    resolving = true;
    error = "";
    try {
      await resolveConflict(conflict.id, side);
      dispatch("resolved");
    } catch (e) {
      error = String(e);
    } finally {
      resolving = false;
    }
  }

  function dismiss() {
    dispatch("dismissed");
  }
</script>

{#if conflict}
  <div class="modal-backdrop" role="dialog" aria-modal="true" aria-labelledby="conflict-title">
    <div class="modal">
      <div class="modal-header">
        <h3 id="conflict-title">Conflict Detected</h3>
        <button class="close-btn" on:click={dismiss} aria-label="Dismiss">✕</button>
      </div>

      <p class="path"><code>{conflict.path}</code></p>

      {#if error}
        <p class="error">{error}</p>
      {/if}

      <div class="versions">
        <div class="version local">
          <h4>Local Version</h4>
          <dl>
            <dt>Modified</dt><dd>{formatDate(conflict.local_mtime)}</dd>
            <dt>Size</dt><dd>{formatBytes(conflict.local_size)}</dd>
          </dl>
          <button
            class="action keep-local"
            on:click={() => choose("local")}
            disabled={resolving}
          >
            Keep Local
          </button>
        </div>

        <div class="version remote">
          <h4>Remote Version</h4>
          <dl>
            <dt>Modified</dt><dd>{formatDate(conflict.remote_mtime)}</dd>
            <dt>Size</dt><dd>{formatBytes(conflict.remote_size)}</dd>
          </dl>
          <button
            class="action keep-remote"
            on:click={() => choose("remote")}
            disabled={resolving}
          >
            Keep Remote
          </button>
        </div>
      </div>

      <p class="hint">
        Policy: <code>{conflict.policy}</code> — choose which version to keep.
      </p>
    </div>
  </div>
{/if}

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .modal {
    background: #fff;
    border-radius: 8px;
    padding: 1.5rem;
    max-width: 560px;
    width: 90%;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.2);
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.75rem;
  }

  h3 {
    margin: 0;
    font-size: 1.1rem;
    color: #c0392b;
  }

  .close-btn {
    background: none;
    border: none;
    font-size: 1rem;
    cursor: pointer;
    color: #666;
    padding: 0.2rem 0.4rem;
  }

  .close-btn:hover {
    color: #333;
  }

  .path {
    font-size: 0.85rem;
    background: #f5f5f5;
    padding: 0.4rem 0.6rem;
    border-radius: 4px;
    margin-bottom: 1rem;
  }

  .versions {
    display: flex;
    gap: 1rem;
    margin-bottom: 1rem;
  }

  .version {
    flex: 1;
    border: 1px solid #e0e0e0;
    border-radius: 6px;
    padding: 0.75rem;
  }

  .version h4 {
    margin: 0 0 0.5rem;
    font-size: 0.9rem;
  }

  dl {
    margin: 0 0 0.75rem;
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 0.2rem 0.5rem;
    font-size: 0.8rem;
  }

  dt {
    color: #666;
  }

  dd {
    margin: 0;
    font-weight: 500;
  }

  .action {
    width: 100%;
    padding: 0.5rem;
    border: none;
    border-radius: 4px;
    font-size: 0.85rem;
    cursor: pointer;
    font-weight: 600;
  }

  .action:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .keep-local {
    background: #3498db;
    color: #fff;
  }

  .keep-local:hover:not(:disabled) {
    background: #2980b9;
  }

  .keep-remote {
    background: #27ae60;
    color: #fff;
  }

  .keep-remote:hover:not(:disabled) {
    background: #229954;
  }

  .hint {
    font-size: 0.78rem;
    color: #888;
    margin: 0;
  }

  .error {
    color: #c0392b;
    font-size: 0.85rem;
    background: #fdecea;
    padding: 0.4rem 0.6rem;
    border-radius: 4px;
    margin-bottom: 0.5rem;
  }
</style>
