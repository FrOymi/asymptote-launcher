<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  interface MinecraftVersion {
    id: string;
    type: string;
    url: string;
  }

  let versions: MinecraftVersion[] = [];
  let selectedVersionUrl: string = '';
  let error: string | null = null;
  let isLoading: boolean = true;

  async function loadVersions(): Promise<void> {
    try {
      versions = await invoke<MinecraftVersion[]>("get_minecraft_versions");
    } catch (err) {
      error = typeof err === "string" ? err : "Не удалось загрузить версии";
    } finally {
      isLoading= false;
    }
  }

  onMount(() => {
    loadVersions();
  });
</script>

<main class="container">
  <h1>Minecraft versions</h1>

  {#if isLoading}
    <p>Загрузка версий от Mojang...</p>
  {:else if error}
    <p style="color: red;">Ошибка: {error}</p>
  {:else}
    <label for="version-select">Select Minecraft version:</label>

    <select id="version-select" bind:value={selectedVersionUrl}>
      <option value="" disabled selected> Select version </option>

      {#each versions as version}
        <option value={version.url}>
          {version.id} ({version.type})
        </option>
      {/each}
    </select>
  {/if}
</main>

<style>

:root {
  font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
  font-size: 16px;
  line-height: 24px;
  font-weight: 400;

  color: #0f0f0f;
  background-color: #f6f6f6;

  font-synthesis: none;
  text-rendering: optimizeLegibility;
  -webkit-font-smoothing: antialiased;
  -moz-osx-font-smoothing: grayscale;
  -webkit-text-size-adjust: 100%;
}

:global(html), :global(body) {
  margin: 0;
  padding: 0;
  background-color: #2f2f2f;
  color: #ffffff;
  height: 100vh;
  width: 100vw;
}

.container {
  margin: 0;
  padding-top: 10vh;
  display: flex;
  flex-direction: column;
  justify-content: center;
  text-align: center;
}

select {
  padding: 8px;
  font-size: 16px;
  width: 100%;
  max-width: 300px;
  display: block;
  margin-top: 5px;
  margin-left: auto;
  margin-right: auto;
}

h1 {
  text-align: center;
}

</style>
