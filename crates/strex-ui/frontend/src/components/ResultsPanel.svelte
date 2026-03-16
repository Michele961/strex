<script lang="ts">
  import type { ResultItem } from '../lib/types'
  import RequestRow from './RequestRow.svelte'
  import IterationSeparator from './IterationSeparator.svelte'

  interface Props {
    items: ResultItem[]
    running: boolean
    total: number
    summary: { passed: number; failed: number; skipped: number; total_duration_ms: number; avg_response_ms: number } | null
  }

  let { items, running, total, summary }: Props = $props()

  type FilterTab = 'all' | 'passed' | 'failed' | 'skipped' | 'errors'
  let activeFilter = $state<FilterTab>('all')

  // Only request-type items, for stats and filtering
  let requestItems = $derived(items.filter((i): i is Extract<ResultItem, { type: 'request' }> => i.type === 'request'))

  let livePassedCount = $derived(requestItems.filter((r) => r.result.passed).length)
  let liveFailedCount = $derived(requestItems.filter((r) => !r.result.passed && r.result.error !== 'skipped').length)
  let liveSkippedCount = $derived(requestItems.filter((r) => r.result.error === 'skipped').length)

  // When filtering, keep iteration separators only in 'all' view
  let filteredItems = $derived(
    activeFilter === 'all'
      ? items
      : items.filter((i) => {
          if (i.type === 'iteration') return false
          const r = i.result
          if (activeFilter === 'passed') return r.passed && !r.error
          if (activeFilter === 'failed') return !r.passed && r.error !== 'skipped' && !r.error
          if (activeFilter === 'skipped') return r.error === 'skipped'
          return !!r.error && r.error !== 'skipped'
        })
  )

  let filteredRequestCount = $derived(filteredItems.filter((i) => i.type === 'request').length)
</script>

<main class="results-panel">
  {#if items.length === 0 && !running}
    <div class="empty-state">
      <p>Configure a collection on the left and click <strong>Run</strong> to start.</p>
    </div>
  {:else}
    <div class="results-header">
      <span class="results-title">Results</span>
      {#if running}
        <span class="running-badge">● Running {requestItems.length}/{total}</span>
      {/if}
    </div>

    <div class="stats-bar">
      <span class="stat">{requestItems.length} requests</span>
      <span class="dot">·</span>
      <span class="stat passed-stat">{livePassedCount} passed</span>
      <span class="dot">·</span>
      <span class="stat failed-stat">{liveFailedCount} failed</span>
      {#if liveSkippedCount > 0}
        <span class="dot">·</span>
        <span class="stat skipped-stat">{liveSkippedCount} skipped</span>
      {/if}
      {#if summary}
        <span class="dot">·</span>
        <span class="stat">{summary.total_duration_ms}ms total</span>
        <span class="dot">·</span>
        <span class="stat">avg {summary.avg_response_ms}ms</span>
      {/if}
    </div>

    <div class="filter-tabs">
      {#each (['all', 'passed', 'failed', 'skipped', 'errors'] as FilterTab[]) as tab}
        <button
          class="filter-tab"
          class:active={activeFilter === tab}
          onclick={() => (activeFilter = tab)}
        >
          {tab.charAt(0).toUpperCase() + tab.slice(1)}
        </button>
      {/each}
    </div>

    <div class="results-list">
      {#each filteredItems as item, i (i)}
        {#if item.type === 'iteration'}
          <IterationSeparator iteration={item.iteration} row={item.row} />
        {:else}
          <RequestRow result={item.result} />
        {/if}
      {/each}
      {#if filteredRequestCount === 0 && requestItems.length > 0}
        <p class="no-match">No {activeFilter} requests.</p>
      {/if}
    </div>
  {/if}
</main>

<style>
  .results-panel {
    flex: 1;
    background: #13132b;
    color: #e0e0e0;
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }

  .empty-state {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #555;
    font-size: 0.95rem;
  }

  .results-header {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 16px 20px 10px;
    border-bottom: 1px solid #1e1e3a;
  }

  .results-title {
    font-weight: 700;
    font-size: 0.9rem;
    color: #aaa;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .running-badge {
    font-size: 0.8rem;
    color: #fca130;
    animation: pulse 1.2s infinite;
  }

  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.5;
    }
  }

  .stats-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 20px;
    font-size: 0.8rem;
    border-bottom: 1px solid #1e1e3a;
    background: #0f0f23;
  }

  .stat {
    color: #888;
  }

  .passed-stat {
    color: #49cc90;
  }

  .failed-stat {
    color: #f93e3e;
  }

  .skipped-stat {
    color: #fbbf24;
  }

  .dot {
    color: #333;
  }

  .filter-tabs {
    display: flex;
    gap: 2px;
    padding: 8px 16px;
    border-bottom: 1px solid #1e1e3a;
  }

  .filter-tab {
    background: none;
    border: none;
    color: #666;
    cursor: pointer;
    padding: 4px 12px;
    font-size: 0.8rem;
    border-radius: 3px;
    transition: background 0.1s;
  }

  .filter-tab:hover {
    background: #1e1e3a;
    color: #bbb;
  }

  .filter-tab.active {
    background: #1e1e3a;
    color: #ff6b35;
    font-weight: 600;
  }

  .results-list {
    flex: 1;
    overflow-y: auto;
  }

  .no-match {
    color: #555;
    font-size: 0.85rem;
    text-align: center;
    padding: 24px;
  }
</style>
