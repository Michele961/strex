<script lang="ts">
  import { onMount } from 'svelte'
  import { fetchHistory, loadHistoryRun } from '../lib/api'
  import type { RunSummary } from '../lib/types'

  interface Props {
    refresh: number
  }

  let { refresh }: Props = $props()

  let collapsed = $state(true)
  let runs = $state<RunSummary[]>([])
  let expandedId = $state<string | null>(null)
  let expandedData = $state<unknown>(null)
  let expandedLoading = $state(false)

  async function loadRuns() {
    try {
      runs = await fetchHistory()
    } catch (e: unknown) {
      console.error('Failed to load history:', e)
    }
  }

  onMount(loadRuns)

  $effect(() => {
    if (refresh > 0) loadRuns()
  })

  async function toggleExpand(id: string) {
    if (expandedId === id) {
      expandedId = null
      expandedData = null
      return
    }
    expandedId = id
    expandedData = null
    expandedLoading = true
    try {
      expandedData = await loadHistoryRun(id)
    } catch (e: unknown) {
      console.error('Failed to load run:', e)
    } finally {
      expandedLoading = false
    }
  }

  function exportRun(id: string) {
    const run = runs.find((r) => r.id === id)
    if (!run) return
    loadHistoryRun(id)
        .then((data: unknown) => {
        const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
        const url = URL.createObjectURL(blob)
        const a = document.createElement('a')
        a.href = url
        a.download = id
        a.click()
        URL.revokeObjectURL(url)
      })
      .catch((e: unknown) => console.error('Export failed:', e))
  }

  function formatTimestamp(ts: string): string {
    try {
      return new Date(ts).toLocaleString()
    } catch {
      return ts
    }
  }
</script>

<div class="history-panel">
  <button class="history-toggle" onclick={() => (collapsed = !collapsed)}>
    <span class="toggle-icon">{collapsed ? '▶' : '▼'}</span>
    History
    {#if runs.length > 0}
      <span class="run-count">{runs.length}</span>
    {/if}
  </button>

  {#if !collapsed}
    <div class="history-list">
      {#if runs.length === 0}
        <div class="empty-history">No runs saved yet.</div>
      {:else}
        {#each runs as run (run.id)}
          <div class="history-item">
            <div
              class="history-row"
              role="button"
              tabindex="0"
              onclick={() => toggleExpand(run.id)}
              onkeydown={(e) => e.key === 'Enter' && toggleExpand(run.id)}
            >
              <span class="run-timestamp">{formatTimestamp(run.timestamp)}</span>
              <span class="run-collection">{run.collection}</span>
              <span class="run-stats">
                <span class="stat-passed">{run.passed}✓</span>
                {#if run.failed > 0}
                  <span class="stat-failed">{run.failed}✗</span>
                {/if}
                {#if run.skipped > 0}
                  <span class="stat-skipped">{run.skipped}⊘</span>
                {/if}
              </span>
              <button
                class="export-btn"
                onclick={(e) => { e.stopPropagation(); exportRun(run.id) }}
                title="Export as JSON"
              >↓</button>
            </div>

            {#if expandedId === run.id}
              <div class="expanded-content">
                {#if expandedLoading}
                  <div class="loading">Loading…</div>
                {:else if expandedData}
                  <pre class="run-json">{JSON.stringify(expandedData, null, 2)}</pre>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      {/if}
    </div>
  {/if}
</div>

<style>
  .history-panel {
    border-top: 1px solid #2a2a4a;
    background: #0f0f24;
    flex-shrink: 0;
  }

  .history-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 8px 16px;
    background: none;
    border: none;
    color: #a0a0c0;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    text-align: left;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .history-toggle:hover {
    color: #e0e0ff;
    background: #1a1a3a;
  }

  .toggle-icon {
    font-size: 10px;
    color: #6060a0;
  }

  .run-count {
    margin-left: 4px;
    padding: 1px 6px;
    background: #2a2a4a;
    border-radius: 10px;
    font-size: 11px;
    color: #a0a0c0;
  }

  .history-list {
    max-height: 240px;
    overflow-y: auto;
  }

  .empty-history {
    padding: 12px 16px;
    color: #606080;
    font-size: 13px;
  }

  .history-item {
    border-top: 1px solid #1e1e38;
  }

  .history-row {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
    padding: 7px 16px;
    background: none;
    border: none;
    color: #c0c0e0;
    font-size: 12px;
    cursor: pointer;
    text-align: left;
    user-select: none;
  }

  .history-row:hover {
    background: #1a1a3a;
  }

  .run-timestamp {
    color: #606080;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .run-collection {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: #d0d0f0;
  }

  .run-stats {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
    font-size: 11px;
  }

  .stat-passed {
    color: #4ade80;
  }

  .stat-failed {
    color: #f87171;
  }

  .stat-skipped {
    color: #fbbf24;
  }

  .export-btn {
    padding: 2px 6px;
    background: #2a2a4a;
    border: 1px solid #3a3a5a;
    border-radius: 4px;
    color: #a0a0c0;
    font-size: 12px;
    cursor: pointer;
    flex-shrink: 0;
  }

  .export-btn:hover {
    background: #3a3a5a;
    color: #e0e0ff;
  }

  .expanded-content {
    padding: 0 16px 12px;
  }

  .loading {
    color: #606080;
    font-size: 12px;
    padding: 4px 0;
  }

  .run-json {
    margin: 0;
    padding: 10px;
    background: #0a0a1e;
    border: 1px solid #2a2a4a;
    border-radius: 4px;
    font-size: 11px;
    color: #a0c0e0;
    overflow: auto;
    max-height: 300px;
    white-space: pre;
  }
</style>
