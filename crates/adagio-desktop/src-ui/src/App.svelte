<script lang="ts">
  import Pairs from "./routes/Pairs.svelte";
  import Dashboard from "./routes/Dashboard.svelte";
  import Conflicts from "./routes/Conflicts.svelte";
  import Settings from "./routes/Settings.svelte";

  type Route = "pairs" | "dashboard" | "conflicts" | "settings";
  let route: Route = "pairs";
</script>

<main>
  <nav>
    <button class:active={route === "pairs"} on:click={() => (route = "pairs")}>Pairs</button>
    <button class:active={route === "dashboard"} on:click={() => (route = "dashboard")}>Dashboard</button>
    <button class:active={route === "conflicts"} on:click={() => (route = "conflicts")}>Conflicts</button>
    <button class:active={route === "settings"} on:click={() => (route = "settings")}>Settings</button>
  </nav>

  {#if route === "pairs"}
    <Pairs />
  {:else if route === "dashboard"}
    <Dashboard />
  {:else if route === "conflicts"}
    <Conflicts />
  {:else}
    <Settings />
  {/if}
</main>

<style>
  :global(*) {
    box-sizing: border-box;
    margin: 0;
    padding: 0;
  }

  :global(body) {
    font-family: system-ui, -apple-system, sans-serif;
    background: #1a1a2e;
    color: #e0e0e0;
  }

  main {
    height: 100vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  nav {
    display: flex;
    gap: 0;
    background: #0f0f23;
    border-bottom: 1px solid #2a2a4a;
    flex-shrink: 0;
  }

  nav button {
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    padding: 0.6rem 1rem;
    color: #888;
    cursor: pointer;
    font-size: 0.85rem;
    transition: color 0.15s;
  }

  nav button:hover {
    color: #ccc;
  }

  nav button.active {
    color: #e0e0e0;
    border-bottom-color: #3498db;
  }
</style>
