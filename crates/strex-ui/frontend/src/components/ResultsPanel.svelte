<script lang="ts">
  import type { RequestResult } from '../lib/types'
  import RequestRow from './RequestRow.svelte'

  interface Props {
    results: RequestResult[]
    running: boolean
    total: number
    summary: { passed: number; failed: number } | null
  }

  let { results, running, total, summary }: Props = $props()
</script>

<main class="results-panel">
  {#if results.length === 0 && !running}
    <div class="empty-state">
      <p>Configure a collection on the left and click <strong>Run</strong> to start.</p>
    </div>
  {:else}
    <div class="results-header">
      <span class="results-title">Results</span>
      {#if running}
        <span class="running-badge">● Running {results.length}/{total}</span>
      {/if}
    </div>

    <div class="results-list">
      {#each results as result, i (i)}
        <RequestRow {result} />
      {/each}
    </div>

    {#if summary}
      <div class="summary" class:all-passed={summary.failed === 0}>
        <span>{results.length} requests</span>
        <span class="dot">·</span>
        <span class="passed-count">{summary.passed} passed</span>
        <span class="dot">·</span>
        <span class="failed-count">{summary.failed} failed</span>
      </div>
    {/if}
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
    padding: 16px 20px;
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
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  .results-list {
    flex: 1;
    overflow-y: auto;
  }

  .summary {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 14px 20px;
    border-top: 1px solid #1e1e3a;
    font-size: 0.875rem;
    background: #1a1a2e;
  }

  .dot { color: #444; }
  .passed-count { color: #49cc90; font-weight: 600; }
  .failed-count { color: #f93e3e; font-weight: 600; }
  .all-passed .failed-count { color: #555; }
</style>
