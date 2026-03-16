<script lang="ts">
  import { connectRun } from './lib/ws'
  import { saveHistory } from './lib/api'
  import type { RunConfig, WsEvent, ResultItem } from './lib/types'
  import ConfigPanel from './components/ConfigPanel.svelte'
  import ResultsPanel from './components/ResultsPanel.svelte'
  import HistoryPanel from './components/HistoryPanel.svelte'

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
</script>

<div class="app">
  <ConfigPanel onRun={handleRun} {running} />
  <div class="main-column">
    <ResultsPanel {items} {running} {total} {summary} />
    <HistoryPanel refresh={historyRefresh} />
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
