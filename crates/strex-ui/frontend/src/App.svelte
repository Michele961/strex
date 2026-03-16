<script lang="ts">
  import { connectRun } from './lib/ws'
  import type { RunConfig, RequestResult, WsEvent } from './lib/types'
  import ConfigPanel from './components/ConfigPanel.svelte'
  import ResultsPanel from './components/ResultsPanel.svelte'

  let running = $state(false)
  let results = $state<RequestResult[]>([])
  let total = $state(0)
  let summary = $state<{
    passed: number
    failed: number
    total_duration_ms: number
    avg_response_ms: number
  } | null>(null)

  function handleRun(config: RunConfig) {
    results = []
    summary = null
    total = 0
    running = true

    connectRun(
      config,
      (event: WsEvent) => {
        if (event.type === 'run_started') {
          total = event.total
        } else if (event.type === 'request_completed') {
          results = [
            ...results,
            {
              name: event.name,
              method: event.method,
              passed: event.passed,
              status: event.status,
              duration_ms: event.duration_ms,
              failures: event.failures,
              error: event.error,
              response_body: event.response_body,
              response_headers: event.response_headers,
            },
          ]
        } else if (event.type === 'run_finished') {
          summary = {
            passed: event.passed,
            failed: event.failed,
            total_duration_ms: event.total_duration_ms,
            avg_response_ms: event.avg_response_ms,
          }
          running = false
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
  <ResultsPanel {results} {running} {total} {summary} />
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
</style>
