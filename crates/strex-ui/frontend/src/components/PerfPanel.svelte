<script lang="ts">
  import type { PerfMetrics, ThresholdResult } from '../lib/types'

  interface Props {
    running: boolean
    started: { vus: number; duration_secs: number; load_profile: string } | null
    tick: {
      elapsed_secs: number
      total_iterations: number
      passed_iterations: number
      failed_iterations: number
      throughput_rps: number
      error_rate_pct: number
      avg_response_ms: number
      p95_response_ms: number
    } | null
    finalMetrics: PerfMetrics | null
    thresholdResults: ThresholdResult[]
    passed: boolean | null
    error: string | null
  }

  let { running, started, tick, finalMetrics, thresholdResults, passed, error }: Props = $props()

  const displayMetrics = $derived(finalMetrics ?? (tick ? {
    total_iterations: tick.total_iterations,
    passed_iterations: tick.passed_iterations,
    failed_iterations: tick.failed_iterations,
    avg_response_ms: tick.avg_response_ms,
    min_response_ms: 0,
    max_response_ms: 0,
    p50_response_ms: 0,
    p95_response_ms: tick.p95_response_ms,
    p99_response_ms: 0,
    error_rate_pct: tick.error_rate_pct,
    throughput_rps: tick.throughput_rps,
    elapsed_secs: tick.elapsed_secs,
  } : null))

  function fmt1(n: number) { return n.toFixed(1) }

  function condLabel(c: string) {
    return c === 'Lt' ? '<' : c === 'Lte' ? '≤' : c === 'Gt' ? '>' : '≥'
  }

  function metricLabel(m: string) {
    const map: Record<string, string> = {
      AvgResponseMs: 'avg_response_ms',
      P95ResponseMs: 'p95_response_ms',
      P99ResponseMs: 'p99_response_ms',
      ErrorRatePct: 'error_rate_pct',
      ThroughputRps: 'throughput_rps',
    }
    return map[m] ?? m
  }
</script>

<div class="perf-panel">
  {#if !started && !running}
    <div class="empty-state">
      <p>Configure and start a performance run from the sidebar.</p>
    </div>
  {:else}
    {#if started}
      <div class="run-header">
        <span class="badge">{started.vus} VU{started.vus === 1 ? '' : 's'}</span>
        <span class="badge">{started.duration_secs}s</span>
        <span class="badge">{started.load_profile === 'ramp_up' ? 'Ramp-up' : 'Fixed'}</span>
        {#if running}
          <span class="badge running-badge">● Running</span>
        {:else if passed === true}
          <span class="badge pass-badge">✓ Passed</span>
        {:else if passed === false}
          <span class="badge fail-badge">✗ Failed</span>
        {/if}
      </div>
    {/if}

    {#if error}
      <div class="error-box">{error}</div>
    {/if}

    {#if displayMetrics}
      <div class="metrics-grid">
        <div class="metric-card">
          <span class="metric-label">Elapsed</span>
          <span class="metric-value">{fmt1(displayMetrics.elapsed_secs)}s</span>
        </div>
        <div class="metric-card">
          <span class="metric-label">Throughput</span>
          <span class="metric-value">{fmt1(displayMetrics.throughput_rps)} req/s</span>
        </div>
        <div class="metric-card">
          <span class="metric-label">Total Iterations</span>
          <span class="metric-value">{displayMetrics.total_iterations}</span>
        </div>
        <div class="metric-card">
          <span class="metric-label">Error Rate</span>
          <span class="metric-value" class:metric-warn={displayMetrics.error_rate_pct > 0}>
            {fmt1(displayMetrics.error_rate_pct)}%
          </span>
        </div>
        <div class="metric-card">
          <span class="metric-label">Avg Response</span>
          <span class="metric-value">{fmt1(displayMetrics.avg_response_ms)} ms</span>
        </div>
        <div class="metric-card">
          <span class="metric-label">p95 Response</span>
          <span class="metric-value">{fmt1(displayMetrics.p95_response_ms)} ms</span>
        </div>
        {#if finalMetrics}
          <div class="metric-card">
            <span class="metric-label">p99 Response</span>
            <span class="metric-value">{fmt1(finalMetrics.p99_response_ms)} ms</span>
          </div>
          <div class="metric-card">
            <span class="metric-label">p50 Response</span>
            <span class="metric-value">{fmt1(finalMetrics.p50_response_ms)} ms</span>
          </div>
          <div class="metric-card">
            <span class="metric-label">Min Response</span>
            <span class="metric-value">{fmt1(finalMetrics.min_response_ms)} ms</span>
          </div>
          <div class="metric-card">
            <span class="metric-label">Max Response</span>
            <span class="metric-value">{fmt1(finalMetrics.max_response_ms)} ms</span>
          </div>
        {/if}
      </div>

      <div class="reliability-row">
        <span class="rel-passed">✓ {displayMetrics.passed_iterations} passed</span>
        <span class="rel-failed">✗ {displayMetrics.failed_iterations} failed</span>
      </div>
    {/if}

    {#if thresholdResults.length > 0}
      <div class="thresholds-section">
        <h3 class="section-title">Thresholds</h3>
        <table class="threshold-table">
          <thead>
            <tr>
              <th></th>
              <th>Metric</th>
              <th>Condition</th>
              <th>Observed</th>
            </tr>
          </thead>
          <tbody>
            {#each thresholdResults as tr}
              <tr class={tr.passed ? 'row-pass' : 'row-fail'}>
                <td class="icon-cell">{tr.passed ? '✓' : '✗'}</td>
                <td>{metricLabel(tr.threshold.metric)}</td>
                <td>{condLabel(tr.threshold.condition)} {tr.threshold.value}</td>
                <td>{fmt1(tr.observed)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}

    {#if running && displayMetrics}
      <div class="progress-hint">Live metrics — updating every second…</div>
    {/if}
  {/if}
</div>

<style>
  .perf-panel {
    flex: 1;
    padding: 28px 32px;
    overflow-y: auto;
    color: #e0e0e0;
  }

  .empty-state {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #555;
    font-size: 0.95rem;
  }

  .run-header {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 24px;
    flex-wrap: wrap;
  }

  .badge {
    padding: 4px 10px;
    border-radius: 4px;
    font-size: 0.8rem;
    font-weight: 600;
    background: #2a2a4a;
    color: #aaa;
  }

  .running-badge {
    background: #1a3a5c;
    color: #60a5fa;
    animation: pulse 1.5s ease-in-out infinite;
  }

  .pass-badge {
    background: #1a3a2a;
    color: #4ade80;
  }

  .fail-badge {
    background: #3a1a1a;
    color: #f87171;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }

  .error-box {
    background: #3a1a1a;
    border: 1px solid #f87171;
    border-radius: 6px;
    padding: 12px 16px;
    color: #f87171;
    font-size: 0.875rem;
    margin-bottom: 20px;
  }

  .metrics-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 12px;
    margin-bottom: 16px;
  }

  .metric-card {
    background: #1a1a2e;
    border: 1px solid #2a2a4a;
    border-radius: 8px;
    padding: 14px 16px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .metric-label {
    font-size: 0.72rem;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .metric-value {
    font-size: 1.4rem;
    font-weight: 700;
    color: #e0e0e0;
    font-variant-numeric: tabular-nums;
  }

  .metric-warn {
    color: #f87171;
  }

  .reliability-row {
    display: flex;
    gap: 24px;
    font-size: 0.875rem;
    margin-bottom: 24px;
    padding: 10px 16px;
    background: #1a1a2e;
    border-radius: 6px;
    border: 1px solid #2a2a4a;
  }

  .rel-passed {
    color: #4ade80;
  }

  .rel-failed {
    color: #f87171;
  }

  .thresholds-section {
    margin-top: 8px;
  }

  .section-title {
    font-size: 0.85rem;
    font-weight: 600;
    color: #888;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    margin: 0 0 12px;
  }

  .threshold-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.875rem;
  }

  .threshold-table th {
    text-align: left;
    padding: 8px 12px;
    color: #666;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    border-bottom: 1px solid #2a2a4a;
  }

  .threshold-table td {
    padding: 10px 12px;
    border-bottom: 1px solid #1e1e38;
    font-variant-numeric: tabular-nums;
  }

  .threshold-table .row-pass {
    color: #4ade80;
  }

  .threshold-table .row-fail {
    color: #f87171;
  }

  .icon-cell {
    font-size: 1rem;
    width: 24px;
  }

  .progress-hint {
    margin-top: 20px;
    font-size: 0.78rem;
    color: #555;
    font-style: italic;
  }
</style>
