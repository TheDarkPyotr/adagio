<script lang="ts">
  import {
    addAccount,
    connectAccountOAuth2,
    createPair,
    type AccountDto,
  } from "../lib/tauri";

  type Step = "account" | "pair" | "preflight" | "syncing";

  let step: Step = "account";
  let error = "";
  let connecting = false;

  // Account form
  let serverUrl = "";
  let username = "";
  let displayName = "";
  let secret = "";
  let authMode: "app-password" | "oauth2" = "oauth2";

  // Created account
  let account: AccountDto | null = null;

  // Pair form
  let localRoot = "";
  let remoteRoot = "/";

  async function handleAddAccount() {
    error = "";
    connecting = true;
    try {
      if (authMode === "oauth2") {
        // Full OAuth2 browser flow — blocks until callback received or error
        account = await connectAccountOAuth2(serverUrl);
      } else {
        account = await addAccount({
          server_url: serverUrl,
          username,
          display_name: displayName || username,
          secret,
        });
      }
      step = "pair";
    } catch (e) {
      error = String(e);
    } finally {
      connecting = false;
    }
  }

  async function handleCreatePair() {
    if (!account) return;
    error = "";
    try {
      await createPair({
        account_id: account.id,
        local_root: localRoot,
        remote_root: remoteRoot,
      });
      step = "preflight";
    } catch (e) {
      error = String(e);
    }
  }

  function handleStartSync() {
    step = "syncing";
  }
</script>

<div class="onboarding">
  {#if step === "account"}
    <h1>Connect your Nextcloud account</h1>
    <form on:submit|preventDefault={handleAddAccount}>
      <label>
        Server URL
        <input
          type="url"
          bind:value={serverUrl}
          placeholder="https://cloud.example.com"
          required
          disabled={connecting}
        />
      </label>

      <label>
        Authentication
        <select bind:value={authMode} disabled={connecting}>
          <option value="oauth2">Sign in with browser (OAuth2)</option>
          <option value="app-password">App password</option>
        </select>
      </label>

      {#if authMode === "app-password"}
        <label>
          Username
          <input type="text" bind:value={username} required disabled={connecting} />
        </label>
        <label>
          Display name (optional)
          <input type="text" bind:value={displayName} placeholder={username} disabled={connecting} />
        </label>
        <label>
          App password
          <input type="password" bind:value={secret} required disabled={connecting} />
        </label>
      {/if}

      {#if connecting && authMode === "oauth2"}
        <div class="waiting-banner">
          <span class="spinner" aria-hidden="true">⏳</span>
          Waiting for browser — complete sign-in then return here.
        </div>
      {/if}

      {#if error}
        <p class="error">{error}</p>
      {/if}

      <button type="submit" class="primary" disabled={connecting}>
        {#if connecting}
          Connecting…
        {:else if authMode === "oauth2"}
          Sign in with browser
        {:else}
          Connect
        {/if}
      </button>
    </form>

  {:else if step === "pair"}
    <h1>Set up sync folder</h1>
    <p>Account: <strong>{account?.display_name}</strong> @ {account?.server_url}</p>
    <form on:submit|preventDefault={handleCreatePair}>
      <label>
        Local folder
        <input
          type="text"
          bind:value={localRoot}
          placeholder="/home/user/Nextcloud"
          required
        />
      </label>
      <label>
        Remote folder
        <input type="text" bind:value={remoteRoot} placeholder="/" />
      </label>
      {#if error}
        <p class="error">{error}</p>
      {/if}
      <div class="actions">
        <button type="button" on:click={() => (step = "account")}>Back</button>
        <button type="submit">Continue</button>
      </div>
    </form>

  {:else if step === "preflight"}
    <h1>Ready to sync</h1>
    <p>
      Adagio will sync <strong>{remoteRoot}</strong> on
      <strong>{account?.server_url}</strong>
      to <strong>{localRoot}</strong>.
    </p>
    <div class="actions">
      <button on:click={() => (step = "pair")}>Back</button>
      <button class="primary" on:click={handleStartSync}>Start sync</button>
    </div>

  {:else if step === "syncing"}
    <h1>Syncing…</h1>
    <p>Your files are being synchronized. You can close this window.</p>
  {/if}
</div>

<style>
  .onboarding {
    max-width: 480px;
    margin: 60px auto;
    padding: 0 24px;
  }

  h1 {
    font-size: 1.5rem;
    margin-bottom: 24px;
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 0.875rem;
    color: #aaa;
  }

  input,
  select {
    padding: 8px 12px;
    border: 1px solid #444;
    border-radius: 6px;
    background: #252540;
    color: #e0e0e0;
    font-size: 1rem;
  }

  input:disabled,
  select:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  button {
    padding: 10px 20px;
    border: none;
    border-radius: 6px;
    background: #444;
    color: #e0e0e0;
    cursor: pointer;
    font-size: 0.95rem;
  }

  button.primary {
    background: #5865f2;
    color: #fff;
  }

  button:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .actions {
    display: flex;
    gap: 12px;
    justify-content: flex-end;
  }

  .waiting-banner {
    background: #1e2a4a;
    border: 1px solid #3a4a7a;
    border-radius: 6px;
    padding: 12px 16px;
    font-size: 0.9rem;
    color: #a0b4e0;
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .spinner {
    font-size: 1.1rem;
  }

  .error {
    color: #f87171;
    font-size: 0.875rem;
  }
</style>
