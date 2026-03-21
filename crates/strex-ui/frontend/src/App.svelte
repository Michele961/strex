<script lang="ts">
  import { connectRun, connectPerf } from './lib/ws'
  import { saveHistory } from './lib/api'
  import type { RunConfig, WsEvent, ResultItem, PerfRunConfig, PerfWsEvent, PerfMetrics, ThresholdResult } from './lib/types'
  import ConfigPanel from './components/ConfigPanel.svelte'
  import ResultsPanel from './components/ResultsPanel.svelte'
  import HistoryPanel from './components/HistoryPanel.svelte'
  import PerfPanel from './components/PerfPanel.svelte'

  let activeTab = $state<'functional' | 'performance'>('functional')

  // ── Functional run state ──────────────────────────────────────────────────
  let running = $state(false)
  let items = $state<ResultItem[]>([])
  let total = $state(0)
  let currentCollection = $state('')
  let summary = $state<{
    passed: number
    failed: number
    skipped: number
    total_duration_ms: number
    avg_response_ms: number
  } | null>(null)
  let historyRefresh = $state(0)

  function handleRun(config: RunConfig) {
    items = []
    summary = null
    total = 0
    running = true
    currentCollection = config.collection

    connectRun(
      config,
      (event: WsEvent) => {
        if (event.type === 'run_started') {
          total = event.total
        } else if (event.type === 'iteration_started') {
          items = [...items, { type: 'iteration', iteration: event.iteration, row: event.row }]
        } else if (event.type === 'request_completed') {
          items = [
            ...items,
            {
              type: 'request',
              result: {
                name: event.name,
                method: event.method,
                url: event.url,
                passed: event.passed,
                status: event.status,
                duration_ms: event.duration_ms,
                failures: event.failures,
                passed_assertions: event.passed_assertions,
                error: event.error,
                response_body: event.response_body,
                response_headers: event.response_headers,
                request_body: event.request_body,
                logs: event.logs,
              },
            },
          ]
        } else if (event.type === 'run_finished') {
          const runSummary = {
            passed: event.passed,
            failed: event.failed,
            skipped: event.skipped,
            total_duration_ms: event.total_duration_ms,
            avg_response_ms: event.avg_response_ms,
          }
          summary = runSummary
          running = false
          saveHistory({
            collection: currentCollection,
            passed: event.passed,
            failed: event.failed,
            skipped: event.skipped,
            run: { items: $state.snapshot(items), summary: runSummary },
          })
            .then(() => { historyRefresh++ })
            .catch((e: unknown) => console.error('Failed to save history:', e))
        } else if (event.type === 'error') {
          console.error('Run error:', event.message)
          running = false
        }
      },
      () => {
        running = false
      }
    )
  }

  // ── Performance run state ─────────────────────────────────────────────────
  let perfRunning = $state(false)
  let perfStarted = $state<{ vus: number; duration_secs: number; load_profile: string } | null>(null)
  let perfTick = $state<{
    elapsed_secs: number
    total_iterations: number
    passed_iterations: number
    failed_iterations: number
    throughput_rps: number
    error_rate_pct: number
    avg_response_ms: number
    p95_response_ms: number
  } | null>(null)
  let perfFinalMetrics = $state<PerfMetrics | null>(null)
  let perfThresholdResults = $state<ThresholdResult[]>([])
  let perfPassed = $state<boolean | null>(null)
  let perfError = $state<string | null>(null)

  function handlePerfRun(config: PerfRunConfig) {
    perfStarted = null
    perfTick = null
    perfFinalMetrics = null
    perfThresholdResults = []
    perfPassed = null
    perfError = null
    perfRunning = true

    connectPerf(
      config,
      (event: PerfWsEvent) => {
        if (event.type === 'Started') {
          perfStarted = { vus: event.vus, duration_secs: event.duration_secs, load_profile: event.load_profile }
        } else if (event.type === 'Tick') {
          perfTick = {
            elapsed_secs: event.elapsed_secs,
            total_iterations: event.total_iterations,
            passed_iterations: event.passed_iterations,
            failed_iterations: event.failed_iterations,
            throughput_rps: event.throughput_rps,
            error_rate_pct: event.error_rate_pct,
            avg_response_ms: event.avg_response_ms,
            p95_response_ms: event.p95_response_ms,
          }
        } else if (event.type === 'Finished') {
          perfFinalMetrics = event.metrics
          perfThresholdResults = event.threshold_results
          perfPassed = event.passed
          perfRunning = false
        } else if (event.type === 'error') {
          perfError = event.message
          perfRunning = false
        }
      },
      () => {
        perfRunning = false
      }
    )
  }
</script>

<div class="app">
  <ConfigPanel
    onRun={handleRun}
    onPerfRun={handlePerfRun}
    {running}
    {perfRunning}
    {activeTab}
    onTabChange={(tab) => (activeTab = tab)}
  />
  <div class="main-column">
    {#if activeTab === 'performance'}
      <PerfPanel
        running={perfRunning}
        started={perfStarted}
        tick={perfTick}
        finalMetrics={perfFinalMetrics}
        thresholdResults={perfThresholdResults}
        passed={perfPassed}
        error={perfError}
      />
    {:else}
      <ResultsPanel {items} {running} {total} {summary} />
      <HistoryPanel refresh={historyRefresh} />
    {/if}
  </div>
</div>

<style>
  :global(*, *::before, *::after) {
    box-sizing: border-box;
  }

  :global(body) {
    margin: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
    background: #13132b;
  }

  .app {
    display: flex;
    height: 100vh;
    overflow: hidden;
  }

  .main-column {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
</style>
