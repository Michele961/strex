<script lang="ts">
  import { onMount } from 'svelte'
  import { listPerfHistory } from '../lib/api'
  import type { PerfRunSummary } from '../lib/types'

  interface Props {
    refresh: number
    onLoad: (id: string) => void
  }

  let { refresh, onLoad }: Props = $props()

  let runs = $state<PerfRunSummary[]>([])

  async function fetchRuns() {
    try {
      runs = await listPerfHistory()
    } catch (e) {
      console.error('Failed to load perf history:', e)
    }
  }

  onMount(fetchRuns)

  $effect(() => {
    if (refresh > 0) {
      fetchRuns()
    }
  })

  function formatDate(iso: string): string {
    return new Date(iso).toLocaleString('en-US', {
      month: 'short',
      day: 'numeric',
      year: 'numeric',
      hour: '2-digit',
      minute: '2-digit',
    })
  }

  function formatDuration(secs: number): string {
    if (secs < 60) return `${secs}s`
    const mins = Math.floor(secs / 60)
    return `${mins} min`
  }
</script>

<div class="history-panel">
  <h2 class="history-heading">Past performance runs</h2>
  {#if runs.length === 0}
    <div class="empty-state">No performance runs yet</div>
  {:else}
    <div class="history-list">
      {#each runs as run}
        <button class="history-item" onclick={() => onLoad(run.id)}>
          <div class="history-header">
            <span class="status-badge {run.passed ? 'passed' : 'failed'}">
              {run.passed ? 'PASS' : 'FAIL'}
            </span>
            <span class="collection-name">{run.collection}</span>
            <span class="run-params">
              · {run.vus} VUs · {formatDuration(run.duration_secs)} · {run.load_profile}
            </span>
          </div>
          <div class="history-stats">
            <span class="timestamp">{formatDate(run.timestamp)}</span>
            <span class="stat-sep">|</span>
            <span class="stat-value">{run.throughput_rps.toFixed(2)} req/s</span>
            <span class="stat-sep">|</span>
            <span class="stat-value">p95: {run.p95_response_ms.toFixed(0)}ms</span>
            <span class="stat-sep">|</span>
            <span class="stat-value {run.error_rate_pct > 0 ? 'stat-error' : ''}">
              error: {run.error_rate_pct.toFixed(2)}%
            </span>
          </div>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .history-panel {
    padding: 1rem;
    background: #12122a;
    border-top: 1px solid #2a2a4a;
    max-height: 200px;
    overflow-y: auto;
  }

  .history-heading {
    font-size: 0.875rem;
    font-weight: 600;
    color: #e0e0e0;
    margin: 0 0 0.75rem 0;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  .empty-state {
    color: #666;
    font-size: 0.875rem;
    padding: 1rem;
    text-align: center;
  }

  .history-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .history-item {
    background: #1a1a38;
    border: 1px solid #2a2a4a;
    border-radius: 6px;
    padding: 0.75rem;
    cursor: pointer;
    text-align: left;
    transition: all 0.15s ease;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .history-item:hover {
    background: #1e1e3e;
    border-color: #3a3a5a;
    transform: translateX(2px);
  }

  .history-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.875rem;
  }

  .status-badge {
    padding: 0.125rem 0.5rem;
    border-radius: 4px;
    font-size: 0.75rem;
    font-weight: 600;
    letter-spacing: 0.5px;
  }

  .status-badge.passed {
    background: #16a34a;
    color: white;
  }

  .status-badge.failed {
    background: #dc2626;
    color: white;
  }

  .collection-name {
    color: #e0e0e0;
    font-weight: 500;
  }

  .run-params {
    color: #999;
    font-size: 0.8125rem;
  }

  .history-stats {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.8125rem;
    color: #999;
  }

  .timestamp {
    color: #888;
  }

  .stat-sep {
    color: #555;
  }

  .stat-value {
    color: #aaa;
  }

  .stat-error {
    color: #f87171;
    font-weight: 500;
  }
</style>
