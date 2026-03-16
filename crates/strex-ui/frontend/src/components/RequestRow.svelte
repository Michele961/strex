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
  let activeTab = $state<'response' | 'headers'>('response')
  let hasDetails = $derived(
    result.failures.length > 0 ||
      !!result.error ||
      !!result.response_body ||
      !!result.response_headers
  )
  let hasTabs = $derived(!!result.response_body || !!result.response_headers)
  let isTruncated = $derived(result.response_body?.endsWith(' [truncated]') ?? false)
</script>

<div class="request-row" class:failed={!result.passed}>
  <div
    class="row-main"
    role="button"
    tabindex="0"
    onclick={() => {
      if (hasDetails) expanded = !expanded
    }}
    onkeydown={(e) => {
      if (e.key === 'Enter' && hasDetails) expanded = !expanded
    }}
  >
    <span class="method" style:color={methodColors[result.method] ?? '#aaa'}>
      {result.method || '—'}
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

      {#if hasTabs}
        <div class="tabs">
          <button
            class="tab"
            class:active={activeTab === 'response'}
            onclick={() => (activeTab = 'response')}
          >
            Response
          </button>
          <button
            class="tab"
            class:active={activeTab === 'headers'}
            onclick={() => (activeTab = 'headers')}
          >
            Headers
          </button>
        </div>

        {#if activeTab === 'response'}
          <pre class="body-pre">{result.response_body ?? '(no body)'}</pre>
          {#if isTruncated}
            <p class="truncated-note">Response truncated at 10 KB</p>
          {/if}
        {:else}
          <table class="headers-table">
            <tbody>
              {#each Object.entries(result.response_headers ?? {}) as [key, value]}
                <tr>
                  <td class="header-key">{key}</td>
                  <td class="header-value">{value}</td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}
      {/if}
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

  .spacer {
    flex: 1;
  }

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

  .failed .indicator {
    color: #f93e3e;
  }
  :not(.failed) .indicator {
    color: #49cc90;
  }

  .chevron {
    color: #666;
    font-size: 0.75rem;
  }

  .details {
    padding: 8px 16px 12px 16px;
    background: #0f0f23;
  }

  .failure-msg,
  .error-msg {
    margin: 4px 0;
    font-size: 0.8rem;
    font-family: monospace;
  }

  .failure-msg {
    color: #f93e3e;
  }
  .error-msg {
    color: #fca130;
  }

  .tabs {
    display: flex;
    gap: 4px;
    margin: 10px 0 6px;
    border-bottom: 1px solid #1e1e3a;
    padding-bottom: 6px;
  }

  .tab {
    background: none;
    border: none;
    color: #888;
    cursor: pointer;
    padding: 4px 10px;
    font-size: 0.8rem;
    border-radius: 3px;
  }

  .tab:hover {
    background: #1e1e3a;
    color: #ccc;
  }

  .tab.active {
    color: #ff6b35;
    font-weight: 600;
  }

  .body-pre {
    margin: 6px 0 0;
    background: #0a0a1a;
    border: 1px solid #1e1e3a;
    border-radius: 4px;
    padding: 10px;
    font-size: 0.75rem;
    font-family: monospace;
    overflow-y: auto;
    max-height: 300px;
    white-space: pre-wrap;
    word-break: break-all;
    color: #c8c8d8;
  }

  .truncated-note {
    margin: 4px 0 0;
    font-size: 0.7rem;
    color: #666;
    font-style: italic;
  }

  .headers-table {
    width: 100%;
    border-collapse: collapse;
    margin-top: 6px;
    font-size: 0.75rem;
  }

  .headers-table tr {
    border-bottom: 1px solid #1a1a30;
  }

  .header-key {
    font-family: monospace;
    font-weight: 600;
    color: #a0a0c0;
    padding: 4px 12px 4px 0;
    white-space: nowrap;
    vertical-align: top;
    width: 35%;
  }

  .header-value {
    color: #c8c8d8;
    padding: 4px 0;
    word-break: break-all;
  }
</style>
