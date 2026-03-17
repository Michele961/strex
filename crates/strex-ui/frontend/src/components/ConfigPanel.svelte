<script lang="ts">
  import { fetchCollections, fetchCollectionRequests, fetchDataPreview } from '../lib/api'
  import type { RunConfig, RequestSequenceItem } from '../lib/types'

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
  let iterations = $state<number | null>(null)
  let delayRequests = $state(0)
  let delayIterations = $state(0)
  let activeTab = $state<'functional' | 'performance'>('functional')
  let requestSequence = $state<RequestSequenceItem[]>([])
  let sequenceLoading = $state(false)
  let dataPreview = $state<Record<string, string>[]>([])
  let dataPreviewError = $state<string | null>(null)
  let dataPreviewLoading = $state(false)

  const methodColors: Record<string, string> = {
    GET: '#61affe',
    POST: '#49cc90',
    PUT: '#fca130',
    PATCH: '#50e3c2',
    DELETE: '#f93e3e',
  }

  // Load collection list on mount
  $effect(() => {
    fetchCollections()
      .then((files) => {
        collections = files
        if (files.length > 0) selectedCollection = files[0]
      })
      .catch((e: unknown) => console.error('Failed to load collections:', e))
  })

  // Load request sequence when selected collection changes
  $effect(() => {
    if (!selectedCollection) {
      requestSequence = []
      return
    }
    sequenceLoading = true
    fetchCollectionRequests(selectedCollection)
      .then((items) => {
        requestSequence = items
      })
      .catch(() => {
        requestSequence = []
      })
      .finally(() => {
        sequenceLoading = false
      })
  })

  // Load data preview when data file changes
  $effect(() => {
    const file = dataFile.trim()
    if (!file) {
      dataPreview = []
      dataPreviewError = null
      return
    }
    dataPreviewLoading = true
    dataPreviewError = null
    fetchDataPreview(file)
      .then((rows) => {
        dataPreview = rows
      })
      .catch((e: unknown) => {
        dataPreview = []
        dataPreviewError = e instanceof Error ? e.message : String(e)
      })
      .finally(() => {
        dataPreviewLoading = false
      })
  })

  const dataPreviewColumns = $derived(
    dataPreview.length > 0 ? Object.keys(dataPreview[0]) : []
  )

  function handleRun() {
    if (!selectedCollection) return
    const iterNum = iterations != null ? Number(iterations) : null
    const reqDelay = Number(delayRequests)
    const iterDelay = Number(delayIterations)
    onRun({
      collection: selectedCollection,
      data: dataFile || undefined,
      concurrency: Number(concurrency),
      fail_fast: failFast,
      max_iterations: dataFile.trim() ? (iterNum ?? undefined) : undefined,
      repeat_iterations: !dataFile.trim() ? (iterNum ?? undefined) : undefined,
      ...(reqDelay > 0 ? { delay_between_requests_ms: reqDelay } : {}),
      ...(iterDelay > 0 ? { delay_between_iterations_ms: iterDelay } : {}),
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

      {#if sequenceLoading}
        <p class="hint">Loading requests…</p>
      {:else if requestSequence.length > 0}
        <ol class="sequence-list">
          {#each requestSequence as item, i}
            <li class="sequence-item">
              <span class="seq-num">{i + 1}.</span>
              <span
                class="seq-method"
                style:color={methodColors[item.method] ?? '#aaa'}
              >{item.method}</span>
              <span class="seq-name">{item.name}</span>
            </li>
          {/each}
        </ol>
      {/if}

      <label class="field">
        <span>Data file <em>(optional)</em></span>
        <input
          type="text"
          placeholder="path/to/data.csv or data.json"
          bind:value={dataFile}
        />
      </label>

      <label class="field">
        <span>Iterations <em>(optional)</em></span>
        <input
          type="number"
          min="1"
          placeholder={dataFile.trim() ? 'All rows' : 'Run once'}
          bind:value={iterations}
        />
      </label>

      {#if dataFile.trim()}
        {#if dataPreviewLoading}
          <p class="hint">Loading preview…</p>
        {:else if dataPreviewError}
          <p class="hint error">{dataPreviewError}</p>
        {:else if dataPreview.length > 0}
          <div class="data-preview">
            <p class="preview-title">Data preview ({dataPreview.length} row{dataPreview.length === 1 ? '' : 's'})</p>
            <div class="preview-table-wrap">
              <table class="preview-table">
                <thead>
                  <tr>
                    {#each dataPreviewColumns as col}
                      <th>{col}</th>
                    {/each}
                  </tr>
                </thead>
                <tbody>
                  {#each dataPreview as row}
                    <tr>
                      {#each dataPreviewColumns as col}
                        <td>{row[col] ?? ''}</td>
                      {/each}
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          </div>
        {/if}
      {/if}

      <label class="field">
        <span>Concurrency</span>
        <input type="number" min="1" max="50" bind:value={concurrency} />
      </label>

      <label class="field">
        <span>Delay between requests <em>(ms)</em></span>
        <input type="number" min="0" bind:value={delayRequests} />
      </label>

      <label class="field">
        <span>Delay between iterations <em>(ms)</em></span>
        <input type="number" min="0" bind:value={delayIterations} />
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
    overflow-y: auto;
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

  .hint.error {
    color: #f87171;
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

  .sequence-list {
    list-style: none;
    margin: 8px 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .sequence-item {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.78rem;
    color: #888;
  }

  .seq-num {
    min-width: 18px;
    color: #555;
    text-align: right;
  }

  .seq-method {
    font-weight: 700;
    font-size: 0.7rem;
    min-width: 40px;
  }

  .seq-name {
    color: #aaa;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .data-preview {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .preview-title {
    margin: 0;
    font-size: 0.78rem;
    color: #888;
  }

  .preview-table-wrap {
    overflow-x: auto;
    border-radius: 4px;
    border: 1px solid #2a2a4a;
  }

  .preview-table {
    border-collapse: collapse;
    font-size: 0.72rem;
    width: 100%;
    color: #ccc;
  }

  .preview-table th {
    background: #0f0f23;
    color: #ff6b35;
    padding: 4px 8px;
    text-align: left;
    white-space: nowrap;
    border-bottom: 1px solid #2a2a4a;
  }

  .preview-table td {
    padding: 4px 8px;
    border-bottom: 1px solid #1e1e38;
    white-space: nowrap;
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .preview-table tr:last-child td {
    border-bottom: none;
  }

  .preview-table tr:nth-child(even) td {
    background: #16162a;
  }
</style>

