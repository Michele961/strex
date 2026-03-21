<script lang="ts">
  import { connectRun, connectPerf } from './lib/ws'
  import { saveHistory, savePerfHistory } from './lib/api'
  import type { RunConfig, WsEvent, ResultItem, PerfRunConfig, PerfWsEvent, PerfMetrics, ThresholdResult, PerfTick, ChartPoint } from './lib/types'
  import ConfigPanel from './components/ConfigPanel.svelte'
  import ResultsPanel from './components/ResultsPanel.svelte'
  import HistoryPanel from './components/HistoryPanel.svelte'
  import PerfView from './components/PerfView.svelte'
  import PerfHistoryPanel from './components/PerfHistoryPanel.svelte'

  // ── Tab state ─────────────────────────────────────────────────────────────
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
      () => { running = false }
    )
  }

  // ── Performance run state ─────────────────────────────────────────────────
  let perfWs = $state<WebSocket | null>(null)
  let perfRunning = $state(false)
  let perfStarted = $state<{ vus: number; duration_secs: number; load_profile: string } | null>(null)
  let perfTick = $state<PerfTick | null>(null)
  let perfFinalMetrics = $state<PerfMetrics | null>(null)
  let perfThresholdResults = $state<ThresholdResult[]>([])
  let perfPassed = $state<boolean | null>(null)
  let perfError = $state<string | null>(null)
  let perfRawTicks = $state<PerfTick[]>([])
  let perfCollection = $state('')
  let perfReplayTimeSeries = $state<ChartPoint[]>([])
  let perfHistoryRefresh = $state(0)
  let perfTickCounter = $state(0)

  function handlePerfRun(config: PerfRunConfig) {
    perfStarted = null
    perfTick = null
    perfFinalMetrics = null
    perfThresholdResults = []
    perfPassed = null
    perfError = null
    perfRunning = true
    perfRawTicks = []
    perfCollection = config.collection
    perfReplayTimeSeries = []

    perfWs = connectPerf(
      config,
      (event: PerfWsEvent) => {
        console.log('[PerfWS]', event.type, event)
        if (event.type === 'Started') {
          perfStarted = { vus: event.vus, duration_secs: event.duration_secs, load_profile: event.load_profile }
        } else if (event.type === 'Tick') {
          // Force complete state update by creating new objects (deep copy)
          perfTick = {
            elapsed_secs: event.elapsed_secs,
            total_iterations: event.total_iterations,
            passed_iterations: event.passed_iterations,
            failed_iterations: event.failed_iterations,
            throughput_rps: event.throughput_rps,
            error_rate_pct: event.error_rate_pct,
            avg_response_ms: event.avg_response_ms,
            p95_response_ms: event.p95_response_ms,
            per_request: [...event.per_request],
          }
          perfRawTicks = [...perfRawTicks, perfTick]
          perfTickCounter++
          console.log('[PerfWS] Updated perfTick:', perfTick, 'counter:', perfTickCounter)
        } else if (event.type === 'Finished') {
          perfFinalMetrics = event.metrics
          perfThresholdResults = event.threshold_results
          perfPassed = event.passed
          perfRunning = false
          perfWs = null
          savePerfHistory({
            collection: perfCollection,
            vus: perfStarted?.vus ?? 0,
            duration_secs: perfStarted?.duration_secs ?? 0,
            load_profile: perfStarted?.load_profile ?? 'fixed',
            metrics: event.metrics,
            threshold_results: event.threshold_results,
            passed: event.passed,
            ticks: perfRawTicks,
          })
            .then(() => { perfHistoryRefresh++ })
            .catch((e: unknown) => console.error('Failed to save perf history:', e))
        } else if (event.type === 'error') {
          perfError = event.message
          perfRunning = false
          perfWs = null
        }
      },
      () => { perfRunning = false; perfWs = null }
    )
  }

  function handlePerfStop() {
    perfWs?.close()
    perfWs = null
    perfRunning = false
  }

  function handlePerfNewRun() {
    perfRawTicks = []
    perfReplayTimeSeries = []
    perfStarted = null
    perfTick = null
    perfFinalMetrics = null
    perfThresholdResults = []
    perfPassed = null
    perfError = null
    perfTickCounter = 0
  }

  async function handlePerfHistoryLoad(id: string) {
    try {
      const { getPerfHistory } = await import('./lib/api')
      const stored = await getPerfHistory(id) as any
      
      perfReplayTimeSeries = stored.ticks.map((t: PerfTick) => ({
        elapsed_secs: t.elapsed_secs,
        throughput_rps: t.throughput_rps,
        avg_response_ms: t.avg_response_ms,
        error_rate_pct: t.error_rate_pct,
        p95_response_ms: t.p95_response_ms,
      }))
      
      perfStarted = {
        vus: stored.vus,
        duration_secs: stored.duration_secs,
        load_profile: stored.load_profile,
      }
      perfFinalMetrics = stored.metrics
      perfThresholdResults = stored.threshold_results
      perfPassed = stored.passed
      perfRawTicks = stored.ticks
    } catch (e) {
      console.error('Failed to load perf history:', e)
    }
  }
</script>

<div class="app">
  <ConfigPanel onRun={handleRun} {running} />

  <div class="main-column">
    <nav class="tab-bar">
      <button
        class="tab-btn"
        class:active={activeTab === 'functional'}
        onclick={() => (activeTab = 'functional')}
      >
        Functional
      </button>
      <button
        class="tab-btn"
        class:active={activeTab === 'performance'}
        onclick={() => (activeTab = 'performance')}
      >
        Performance
      </button>
    </nav>

    {#if activeTab === 'performance'}
      <div class="perf-area">
        <PerfView
          onRun={handlePerfRun}
          onStop={handlePerfStop}
          onNewRun={handlePerfNewRun}
          running={perfRunning}
          started={perfStarted}
          tick={perfTick}
          finalMetrics={perfFinalMetrics}
          thresholdResults={perfThresholdResults}
          passed={perfPassed}
          error={perfError}
          initialTimeSeries={perfReplayTimeSeries}
          tickCounter={perfTickCounter}
        />
      </div>
      {#if !perfRunning}
        <PerfHistoryPanel refresh={perfHistoryRefresh} onLoad={handlePerfHistoryLoad} />
      {/if}
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

  .tab-bar {
    display: flex;
    gap: 4px;
    padding: 0 20px;
    border-bottom: 1px solid #1e1e38;
    background: #13132b;
    flex-shrink: 0;
  }

  .tab-btn {
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    color: #888;
    cursor: pointer;
    padding: 12px 16px;
    font-size: 0.875rem;
    font-weight: 500;
    transition: color 0.15s, border-color 0.15s;
    margin-bottom: -1px;
  }

  .tab-btn:hover {
    color: #ccc;
  }

  .tab-btn.active {
    color: #e0e0e0;
    border-bottom-color: #ff6b35;
    font-weight: 600;
  }

  .perf-area {
    flex: 1;
    overflow-y: auto;
  }
</style>
