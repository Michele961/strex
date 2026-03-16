<script lang="ts">
  import type { RequestResult, ConsoleLog } from '../lib/types'

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
  let activeTab = $state<'request' | 'response' | 'headers' | 'console' | 'assertions'>('response')
  let isTruncated = $derived(result.response_body?.endsWith(' [truncated]') ?? false)

  interface ParsedFailure {
    kind: string
    expected: string
    actual: string
  }

  // Parse the stable failure string format produced by ws.rs format_failure():
  //   "status expected 200, got 404"
  //   "jsonPath $.id expected octocat, got null"
  //   "header expected application/json, got text/plain"
  // Falls back to treating the whole string as a freeform message (script assertions).
  function parseFailure(s: string): ParsedFailure {
    const jsonPathMatch = s.match(/^jsonPath (\S+) expected (.*?), got (.*)$/)
    if (jsonPathMatch) return { kind: jsonPathMatch[1], expected: jsonPathMatch[2], actual: jsonPathMatch[3] }
    const m = s.match(/^(\w+) expected (.*?), got (.*)$/)
    if (m) return { kind: m[1], expected: m[2], actual: m[3] }
    return { kind: 'assertion', expected: s, actual: '' }
  }

  let parsedFailures = $derived(result.failures.map(parseFailure))

  // Status badge label and variant
  let badge = $derived(
    result.error === 'skipped'
      ? { label: '⊘ skipped', variant: 'skipped' as const }
      : result.error
        ? { label: '⚠ error', variant: 'error' as const }
        : result.passed
          ? { label: '✓ passed', variant: 'passed' as const }
          : {
              label: `✗ ${result.failures.length} failed`,
              variant: 'failed' as const,
            }
  )

  let hasInlineContent = $derived(result.error !== 'skipped' && (!result.passed || !!result.error))
  let hasLogs = $derived(result.logs.length > 0)
  let hasAssertions = $derived(result.passed_assertions.length > 0 || result.failures.length > 0)
</script>

<div class="request-row" class:failed={!result.passed && result.error !== 'skipped'} class:errored={!!result.error && result.error !== 'skipped'} class:skipped={result.error === 'skipped'}>
  <!-- Clickable header row -->
  <div
    class="row-main"
    role="button"
    tabindex="0"
    onclick={() => (expanded = !expanded)}
    onkeydown={(e) => {
      if (e.key === 'Enter') expanded = !expanded
    }}
  >
    <div class="row-left">
      <div class="row-top">
        <span class="method" style:color={methodColors[result.method] ?? '#aaa'}>
          {result.method || '—'}
        </span>
        <span class="name">{result.name}</span>
      </div>
      {#if result.url}
        <span class="url">{result.url}</span>
      {/if}
    </div>
    <span class="spacer"></span>
    {#if result.status}
      <span class="status" class:status-ok={result.status < 400} class:status-err={result.status >= 400}>
        {result.status}
      </span>
    {/if}
    <span class="duration">{result.duration_ms}ms</span>
    <span class="badge badge--{badge.variant}">{badge.label}</span>
    <span class="chevron">{expanded ? '▾' : '▸'}</span>
  </div>

  <!-- Inline assertion failures — always visible, no click needed -->
  {#if hasInlineContent}
    <div class="inline-failures">
      {#if result.error}
        <div class="assertion-card assertion-card--error">
          <span class="assertion-kind">network error</span>
          <span class="assertion-message">{result.error}</span>
        </div>
      {/if}
      {#each parsedFailures as f}
        <div class="assertion-card">
          <span class="assertion-kind">{f.kind}</span>
          <div class="assertion-diff">
            <span class="diff-label">expected</span>
            <code class="diff-value diff-value--expected">{f.expected}</code>
            {#if f.actual}
              <span class="diff-label">got</span>
              <code class="diff-value diff-value--actual">{f.actual}</code>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/if}

  <!-- Expandable request / response / headers tabs -->
  {#if expanded}
    <div class="details">
      <div class="tabs">
        <button
          class="tab"
          class:active={activeTab === 'request'}
          onclick={() => (activeTab = 'request')}
        >
          Request
        </button>
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
        {#if hasLogs}
          <button
            class="tab"
            class:active={activeTab === 'console'}
            onclick={() => (activeTab = 'console')}
          >
            Console
          </button>
        {/if}
        {#if hasAssertions}
          <button
            class="tab"
            class:active={activeTab === 'assertions'}
            onclick={() => (activeTab = 'assertions')}
          >
            Assertions
          </button>
        {/if}
      </div>

      {#if activeTab === 'request'}
        {#if result.request_body !== null}
          <pre class="body-pre">{result.request_body}</pre>
        {:else}
          <p class="no-body">No request body</p>
        {/if}
      {:else if activeTab === 'response'}
        <pre class="body-pre">{result.response_body ?? '(no body)'}</pre>
        {#if isTruncated}
          <p class="truncated-note">Response truncated at 10 KB</p>
        {/if}
      {:else if activeTab === 'headers'}
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
      {:else if activeTab === 'console'}
        <div class="console-log-list">
          {#each result.logs as entry}
            <div class="console-entry console-entry--{entry.level}">
              <span class="console-level">{entry.level}</span>
              <span class="console-message">{entry.message}</span>
            </div>
          {/each}
        </div>
      {:else if activeTab === 'assertions'}
        <div class="assertions-list">
          {#each result.passed_assertions as desc}
            <div class="assertion-row assertion-row--passed">
              <span class="assertion-check">✓</span>
              <span class="assertion-desc">{desc}</span>
            </div>
          {/each}
          {#each parsedFailures as f}
            <div class="assertion-row assertion-row--failed">
              <span class="assertion-check">✗</span>
              <span class="assertion-desc">
                {f.kind}{f.actual ? ` — expected ${f.expected}, got ${f.actual}` : f.expected !== f.kind ? ` — ${f.expected}` : ''}
              </span>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .request-row {
    border-bottom: 1px solid #1e1e3a;
    font-size: 0.875rem;
    border-left: 3px solid transparent;
    transition: border-color 0.1s;
  }

  .request-row.failed {
    border-left-color: #f93e3e;
  }

  .request-row.errored {
    border-left-color: #fca130;
  }

  .row-main {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px 10px 13px;
    cursor: pointer;
  }

  .row-main:hover {
    background: #16162e;
  }

  .row-left {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .row-top {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .url {
    font-size: 0.72rem;
    color: #555;
    font-family: 'SF Mono', 'Fira Code', monospace;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 420px;
  }

  .method {
    font-weight: 700;
    font-size: 0.75rem;
    min-width: 52px;
    letter-spacing: 0.03em;
  }

  .name {
    color: #d8d8e8;
  }

  .spacer {
    flex: 1;
  }

  .status {
    font-size: 0.78rem;
    font-family: monospace;
    min-width: 36px;
    text-align: right;
    font-weight: 600;
  }

  .status-ok {
    color: #49cc90;
  }

  .status-err {
    color: #f93e3e;
  }

  .duration {
    color: #555;
    font-size: 0.75rem;
    min-width: 52px;
    text-align: right;
  }

  /* Status badge */
  .badge {
    font-size: 0.7rem;
    font-weight: 700;
    padding: 2px 8px;
    border-radius: 10px;
    white-space: nowrap;
    letter-spacing: 0.02em;
  }

  .badge--passed {
    background: rgba(73, 204, 144, 0.12);
    color: #49cc90;
    border: 1px solid rgba(73, 204, 144, 0.25);
  }

  .badge--failed {
    background: rgba(249, 62, 62, 0.12);
    color: #f93e3e;
    border: 1px solid rgba(249, 62, 62, 0.25);
  }

  .badge--error {
    background: rgba(252, 161, 48, 0.12);
    color: #fca130;
    border: 1px solid rgba(252, 161, 48, 0.25);
  }

  .badge--skipped {
    background: rgba(251, 191, 36, 0.1);
    color: #fbbf24;
    border: 1px solid rgba(251, 191, 36, 0.2);
  }

  .request-row.skipped {
    border-left-color: #fbbf24;
    opacity: 0.65;
  }

  .chevron {
    color: #444;
    font-size: 0.75rem;
  }

  /* Inline assertion failure cards */
  .inline-failures {
    padding: 0 16px 10px 16px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .assertion-card {
    display: flex;
    align-items: baseline;
    gap: 12px;
    padding: 7px 12px;
    background: #0a0a1a;
    border-radius: 4px;
    border-left: 3px solid #f93e3e;
  }

  .assertion-card--error {
    border-left-color: #fca130;
  }

  .assertion-kind {
    font-size: 0.68rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: #888;
    white-space: nowrap;
    min-width: 60px;
  }

  .assertion-message {
    font-size: 0.78rem;
    color: #fca130;
    font-family: 'SF Mono', 'Fira Code', monospace;
  }

  .assertion-diff {
    display: flex;
    align-items: baseline;
    gap: 8px;
    flex-wrap: wrap;
  }

  .diff-label {
    font-size: 0.68rem;
    color: #555;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    white-space: nowrap;
  }

  .diff-value {
    font-family: 'SF Mono', 'Fira Code', monospace;
    font-size: 0.78rem;
    padding: 1px 6px;
    border-radius: 3px;
  }

  .diff-value--expected {
    background: rgba(73, 204, 144, 0.1);
    color: #49cc90;
    border: 1px solid rgba(73, 204, 144, 0.2);
  }

  .diff-value--actual {
    background: rgba(249, 62, 62, 0.1);
    color: #f93e3e;
    border: 1px solid rgba(249, 62, 62, 0.2);
  }

  /* Expandable details panel */
  .details {
    padding: 0 16px 12px 16px;
    background: #0f0f23;
  }

  .tabs {
    display: flex;
    gap: 4px;
    padding: 8px 0 6px;
    border-bottom: 1px solid #1e1e3a;
    margin-bottom: 6px;
  }

  .tab {
    background: none;
    border: none;
    color: #666;
    cursor: pointer;
    padding: 4px 10px;
    font-size: 0.8rem;
    border-radius: 3px;
    transition: background 0.1s;
  }

  .tab:hover {
    background: #1e1e3a;
    color: #bbb;
  }

  .tab.active {
    color: #ff6b35;
    font-weight: 600;
  }

  .body-pre {
    margin: 0;
    background: #0a0a1a;
    border: 1px solid #1e1e3a;
    border-radius: 4px;
    padding: 10px;
    font-size: 0.75rem;
    font-family: 'SF Mono', 'Fira Code', monospace;
    overflow-y: auto;
    max-height: 300px;
    white-space: pre-wrap;
    word-break: break-all;
    color: #c8c8d8;
  }

  .truncated-note {
    margin: 4px 0 0;
    font-size: 0.7rem;
    color: #555;
    font-style: italic;
  }

  .no-body {
    margin: 4px 0 0;
    font-size: 0.75rem;
    color: #555;
    font-style: italic;
  }

  .headers-table {
    width: 100%;
    border-collapse: collapse;
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

  .console-log-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: #0a0a1a;
    border: 1px solid #1e1e3a;
    border-radius: 4px;
    padding: 6px 0;
    font-family: 'SF Mono', 'Fira Code', monospace;
    font-size: 0.75rem;
    max-height: 300px;
    overflow-y: auto;
  }

  .console-entry {
    display: flex;
    align-items: baseline;
    gap: 10px;
    padding: 3px 10px;
  }

  .console-entry--log .console-message { color: #888; }
  .console-entry--warn .console-message { color: #fca130; }
  .console-entry--error .console-message { color: #f93e3e; }

  .console-level {
    font-size: 0.65rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    min-width: 36px;
    white-space: nowrap;
  }

  .console-entry--log .console-level { color: #444; }
  .console-entry--warn .console-level { color: #c07010; }
  .console-entry--error .console-level { color: #c02020; }

  .console-message {
    white-space: pre-wrap;
    word-break: break-all;
  }

  /* Assertions tab */
  .assertions-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: #0a0a1a;
    border: 1px solid #1e1e3a;
    border-radius: 4px;
    padding: 6px 0;
    font-size: 0.78rem;
    font-family: 'SF Mono', 'Fira Code', monospace;
    max-height: 300px;
    overflow-y: auto;
  }

  .assertion-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
    padding: 3px 12px;
  }

  .assertion-check {
    font-size: 0.8rem;
    font-weight: 700;
    min-width: 14px;
    flex-shrink: 0;
  }

  .assertion-row--passed .assertion-check { color: #49cc90; }
  .assertion-row--failed .assertion-check { color: #f93e3e; }

  .assertion-desc {
    color: #c8c8d8;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .assertion-row--failed .assertion-desc { color: #f98080; }
</style>
