<script lang="ts">
  import type { RequestResult } from '../lib/types'

  interface Props {
    result: RequestResult
  }

  let { result }: Props = $props()

  const methodColors: Record<string, string> = {
    GET: '#61affe',
    POST: '#49cc90',
    PUT: '#fca130',
    PATCH: '#50e3c2',
    DELETE: '#f93e3e',
  }

  let expanded = $state(false)
  let hasDetails = $derived(result.failures.length > 0 || !!result.error)
</script>

<div class="request-row" class:failed={!result.passed}>
  <div
    class="row-main"
    role="button"
    tabindex="0"
    onclick={() => { if (hasDetails) expanded = !expanded }}
    onkeydown={(e) => { if (e.key === 'Enter' && hasDetails) expanded = !expanded }}
  >
    <span class="method" style:color={methodColors[result.method] ?? '#aaa'}>
      {result.method}
    </span>
    <span class="name">{result.name}</span>
    <span class="spacer"></span>
    {#if result.status}
      <span class="status">{result.status}</span>
    {/if}
    <span class="duration">{result.duration_ms}ms</span>
    <span class="indicator">{result.passed ? '✓' : '✗'}</span>
    {#if hasDetails}
      <span class="chevron">{expanded ? '▾' : '▸'}</span>
    {/if}
  </div>

  {#if expanded && hasDetails}
    <div class="details">
      {#if result.error}
        <p class="error-msg">error: {result.error}</p>
      {/if}
      {#each result.failures as failure}
        <p class="failure-msg">assertion failed: {failure}</p>
      {/each}
    </div>
  {/if}
</div>

<style>
  .request-row {
    border-bottom: 1px solid #1e1e3a;
    font-size: 0.875rem;
  }

  .row-main {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px;
    cursor: default;
  }

  .method {
    font-weight: 700;
    font-size: 0.75rem;
    min-width: 52px;
  }

  .name {
    color: #e0e0e0;
    flex: 1;
  }

  .spacer { flex: 1; }

  .status {
    color: #888;
    font-size: 0.8rem;
    min-width: 36px;
    text-align: right;
  }

  .duration {
    color: #666;
    font-size: 0.75rem;
    min-width: 52px;
    text-align: right;
  }

  .indicator {
    font-size: 1rem;
    min-width: 20px;
    text-align: center;
  }

  .failed .indicator { color: #f93e3e; }
  :not(.failed) .indicator { color: #49cc90; }

  .chevron {
    color: #666;
    font-size: 0.75rem;
  }

  .details {
    padding: 8px 16px 12px 78px;
    background: #0f0f23;
  }

  .failure-msg, .error-msg {
    margin: 4px 0;
    font-size: 0.8rem;
    font-family: monospace;
  }

  .failure-msg { color: #f93e3e; }
  .error-msg { color: #fca130; }
</style>
