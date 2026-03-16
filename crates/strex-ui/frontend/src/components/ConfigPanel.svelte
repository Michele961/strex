<script lang="ts">
  import { fetchCollections } from '../lib/api'
  import type { RunConfig } from '../lib/types'

  interface Props {
    onRun: (config: RunConfig) => void
    running: boolean
  }

  let { onRun, running }: Props = $props()

  let collections = $state<string[]>([])
  let selectedCollection = $state('')
  let dataFile = $state('')
  let concurrency = $state(1)
  let failFast = $state(false)
  let activeTab = $state<'functional' | 'performance'>('functional')

  $effect(() => {
    fetchCollections()
      .then((files) => {
        collections = files
        if (files.length > 0) selectedCollection = files[0]
      })
      .catch((e: unknown) => console.error('Failed to load collections:', e))
  })

  function handleRun() {
    if (!selectedCollection) return
    onRun({
      collection: selectedCollection,
      data: dataFile || undefined,
      concurrency,
      fail_fast: failFast,
    })
  }
</script>

<aside class="config-panel">
  <header class="panel-header">
    <h1>strex</h1>
    <p class="subtitle">API Collection Runner</p>
  </header>

  <nav class="tabs">
    <button
      class="tab"
      class:active={activeTab === 'functional'}
      onclick={() => (activeTab = 'functional')}
    >
      Functional
    </button>
    <button class="tab" disabled title="Coming soon">
      Performance
    </button>
  </nav>

  {#if activeTab === 'functional'}
    <div class="form">
      <label class="field">
        <span>Collection</span>
        {#if collections.length > 0}
          <select bind:value={selectedCollection}>
            {#each collections as file}
              <option value={file}>{file}</option>
            {/each}
          </select>
        {:else}
          <p class="hint">No .yaml files found in the current directory.</p>
        {/if}
      </label>

      <label class="field">
        <span>Data file <em>(optional)</em></span>
        <input
          type="text"
          placeholder="path/to/data.csv or data.json"
          bind:value={dataFile}
        />
      </label>

      <label class="field">
        <span>Concurrency</span>
        <input type="number" min="1" max="50" bind:value={concurrency} />
      </label>

      <label class="field checkbox">
        <input type="checkbox" bind:checked={failFast} />
        <span>Fail fast</span>
      </label>

      <button
        class="run-button"
        onclick={handleRun}
        disabled={running || !selectedCollection}
      >
        {running ? 'Running…' : 'Run'}
      </button>
    </div>
  {/if}
</aside>

<style>
  .config-panel {
    width: 280px;
    min-width: 260px;
    background: #1a1a2e;
    color: #e0e0e0;
    display: flex;
    flex-direction: column;
    padding: 24px 20px;
    gap: 20px;
    border-right: 1px solid #2a2a4a;
    height: 100vh;
    box-sizing: border-box;
  }

  .panel-header h1 {
    margin: 0;
    font-size: 1.5rem;
    color: #ff6b35;
    font-weight: 700;
    letter-spacing: 0.05em;
  }

  .subtitle {
    margin: 4px 0 0;
    font-size: 0.75rem;
    color: #888;
  }

  .tabs {
    display: flex;
    gap: 8px;
    border-bottom: 1px solid #2a2a4a;
    padding-bottom: 12px;
  }

  .tab {
    background: none;
    border: none;
    color: #aaa;
    cursor: pointer;
    padding: 6px 12px;
    border-radius: 4px;
    font-size: 0.875rem;
    transition: background 0.15s;
  }

  .tab:hover:not(:disabled) {
    background: #2a2a4a;
    color: #fff;
  }

  .tab.active {
    background: #2a2a4a;
    color: #ff6b35;
    font-weight: 600;
  }

  .tab:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 0.85rem;
  }

  .field span {
    color: #bbb;
  }

  .field em {
    color: #666;
    font-style: normal;
  }

  .field select,
  .field input[type='text'],
  .field input[type='number'] {
    background: #0f0f23;
    border: 1px solid #333;
    border-radius: 4px;
    color: #e0e0e0;
    padding: 8px 10px;
    font-size: 0.875rem;
    width: 100%;
    box-sizing: border-box;
  }

  .field.checkbox {
    flex-direction: row;
    align-items: center;
    gap: 10px;
  }

  .hint {
    color: #666;
    font-size: 0.8rem;
    margin: 0;
  }

  .run-button {
    margin-top: 8px;
    padding: 12px;
    background: #ff6b35;
    color: white;
    border: none;
    border-radius: 6px;
    font-size: 1rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
  }

  .run-button:hover:not(:disabled) {
    background: #ff8555;
  }

  .run-button:disabled {
    background: #444;
    cursor: not-allowed;
  }
</style>
