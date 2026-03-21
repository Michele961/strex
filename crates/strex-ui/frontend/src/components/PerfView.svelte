<script lang="ts">
  import { onMount } from 'svelte'
  import { fetchCollections } from '../lib/api'
  import type { PerfRunConfig, PerfMetrics, ThresholdResult, ChartPoint } from '../lib/types'

  interface Props {
    onRun: (config: PerfRunConfig) => void
    onStop: () => void
    onNewRun: () => void
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
      per_request: Array<{
        name: string
        total: number
        passed: number
        failed: number
        throughput_rps: number
        avg_response_ms: number
        error_rate_pct: number
      }>
    } | null
    finalMetrics: PerfMetrics | null
    thresholdResults: ThresholdResult[]
    passed: boolean | null
    error: string | null
    initialTimeSeries?: ChartPoint[]
    tickCounter: number
  }

  let { onRun, onStop, onNewRun, running, started, tick, finalMetrics, thresholdResults, passed, error, initialTimeSeries, tickCounter }: Props =
    $props()

  // ── Setup form state ──────────────────────────────────────────────────────
  let collections = $state<string[]>([])
  let selectedCollection = $state('')
  let vus = $state(20)
  let durationMins = $state(10)
  let loadProfile = $state<'fixed' | 'ramp_up'>('fixed')
  let initialVus = $state(5)
  let dataFile = $state('')
  let thresholdsRaw = $state('')
  let showThresholds = $state(false)

  // ── Chart time-series ─────────────────────────────────────────────────────
  let timeSeries = $state<ChartPoint[]>([])

  onMount(() => {
    fetchCollections()
      .then((files) => {
        collections = files
        if (files.length > 0) selectedCollection = files[0]
      })
      .catch((e: unknown) => console.error('Failed to load collections:', e))
  })

  $effect(() => {
    if (initialTimeSeries && initialTimeSeries.length > 0) {
      timeSeries = initialTimeSeries
    }
  })

  $effect(() => {
    if (tick && tickCounter > 0) {
      const t = tick
      const lastPoint = timeSeries[timeSeries.length - 1]
      // Only append if this is a new tick (different elapsed time)
      if (!lastPoint || lastPoint.elapsed_secs !== t.elapsed_secs) {
        timeSeries = [
          ...timeSeries,
          {
            elapsed_secs: t.elapsed_secs,
            throughput_rps: t.throughput_rps,
            avg_response_ms: t.avg_response_ms,
            error_rate_pct: t.error_rate_pct,
            p95_response_ms: t.p95_response_ms,
          },
        ]
      }
    }
  })

  // ── Simple derived values ─────────────────────────────────────────────────
  const durationSecs = $derived(durationMins * 60)

  const displayMetrics = $derived(
    finalMetrics ??
      (tick
        ? {
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
            per_request: [],
          }
        : started
        ? {
            total_iterations: 0,
            passed_iterations: 0,
            failed_iterations: 0,
            avg_response_ms: 0,
            min_response_ms: 0,
            max_response_ms: 0,
            p50_response_ms: 0,
            p95_response_ms: 0,
            p99_response_ms: 0,
            error_rate_pct: 0,
            throughput_rps: 0,
            elapsed_secs: 0,
            per_request: [],
          }
        : null)
  )

  const maxThroughput = $derived(Math.max(1, ...timeSeries.map((p) => p.throughput_rps)))
  const maxResponse = $derived(Math.max(1, ...timeSeries.map((p) => p.avg_response_ms)))

  // ── SVG load profile preview constants ───────────────────────────────────
  const PV_W = 500
  const PV_H = 90
  const PL = 44
  const PR = 12
  const PT = 8
  const PB = 28
  const pW = PV_W - PL - PR
  const pH = PV_H - PT - PB

  // ── SVG live chart constants ──────────────────────────────────────────────
  const CW = 800
  const CH = 210
  const ML = 8
  const MR = 8
  const MT = 12
  const MB = 44
  const cW = CW - ML - MR
  const cH = CH - MT - MB

  // ── Derived SVG data ──────────────────────────────────────────────────────
  const profilePoints = $derived.by(() => {
    if (loadProfile === 'fixed') {
      return `${PL},${PT} ${PL + pW},${PT} ${PL + pW},${PT + pH} ${PL},${PT + pH}`
    }
    const safeInit = Math.min(initialVus, vus)
    const yStart = PT + pH - (safeInit / vus) * pH
    const midX = PL + pW / 2
    return `${PL},${yStart} ${midX},${PT} ${PL + pW},${PT} ${PL + pW},${PT + pH} ${PL},${PT + pH}`
  })

  const profileLinePts = $derived.by(() => {
    if (loadProfile === 'fixed') {
      return `${PL},${PT} ${PL + pW},${PT}`
    }
    const safeInit = Math.min(initialVus, vus)
    const yStart = PT + pH - (safeInit / vus) * pH
    const midX = PL + pW / 2
    return `${PL},${yStart} ${midX},${PT} ${PL + pW},${PT}`
  })

  const profileDesc = $derived.by(() => {
    if (loadProfile === 'fixed') {
      return `All ${vus} virtual users start simultaneously and run for the entire ${durationMins} minute${durationMins === 1 ? '' : 's'}, each executing all requests sequentially.`
    }
    const safeInit = Math.min(initialVus, vus)
    const halfMins = durationMins / 2
    return `${safeInit} virtual user${safeInit === 1 ? '' : 's'} start immediately, ramp up to ${vus} over the first ${halfMins} minute${halfMins === 1 ? '' : 's'}, then maintain ${vus} for the remaining ${halfMins} minute${halfMins === 1 ? '' : 's'}, each executing all requests sequentially.`
  })

  function xOf(elapsed: number, totalDur: number): number {
    return ML + (totalDur > 0 ? (elapsed / totalDur) * cW : 0)
  }

  function yOf(val: number, max: number): number {
    return MT + cH - (max > 0 ? Math.min(1, val / max) * cH : 0)
  }

  const throughputPts = $derived.by(() => {
    const dur = started?.duration_secs ?? 1
    return timeSeries.map((p) => `${xOf(p.elapsed_secs, dur)},${yOf(p.throughput_rps, maxThroughput)}`).join(' ')
  })

  const responsePts = $derived.by(() => {
    const dur = started?.duration_secs ?? 1
    return timeSeries.map((p) => `${xOf(p.elapsed_secs, dur)},${yOf(p.avg_response_ms, maxResponse)}`).join(' ')
  })

  const errorPts = $derived.by(() => {
    const dur = started?.duration_secs ?? 1
    return timeSeries.map((p) => `${xOf(p.elapsed_secs, dur)},${yOf(p.error_rate_pct, 100)}`).join(' ')
  })

  const vuLinePts = $derived.by(() => {
    if (!started) return ''
    const dur = started.duration_secs
    const maxVu = started.vus
    
    if (started.load_profile === 'fixed') {
      const y = yOf(maxVu, maxVu)
      return `${ML},${y} ${ML + cW},${y}`
    } else {
      const halfDur = dur / 2
      const y1 = MT + cH
      const yMid = yOf(maxVu, maxVu)
      const xMid = xOf(halfDur, dur)
      const xEnd = ML + cW
      return `${ML},${y1} ${xMid},${yMid} ${xEnd},${yMid}`
    }
  })

  const maxErrorRate = 100
  const maxVuScale = $derived(started?.vus ?? 1)

  // ── Helpers ───────────────────────────────────────────────────────────────
  function fmtNum(n: number): string {
    return n >= 1000 ? n.toLocaleString() : n.toFixed(n < 10 ? 2 : 1)
  }

  function fmtMs(n: number): string {
    return n < 1 ? '<1' : `${Math.round(n)}`
  }

  const timeRemainingStr = $derived.by(() => {
    if (!started) return ''
    const elapsed = tick?.elapsed_secs ?? 0
    const rem = Math.max(0, started.duration_secs - elapsed)
    const m = Math.floor(rem / 60)
    const s = Math.floor(rem % 60)
    return `${m}:${s.toString().padStart(2, '0')} left`
  })

  const elapsedPct = $derived.by(() => {
    if (!started || !tick) return 0
    return Math.min(100, (tick.elapsed_secs / started.duration_secs) * 100)
  })

  function condLabel(c: string): string {
    return c === 'Lt' ? '<' : c === 'Lte' ? '≤' : c === 'Gt' ? '>' : '≥'
  }

  function metricLabel(m: string): string {
    const map: Record<string, string> = {
      AvgResponseMs: 'avg_response_ms',
      P95ResponseMs: 'p95_response_ms',
      P99ResponseMs: 'p99_response_ms',
      ErrorRatePct: 'error_rate_pct',
      ThroughputRps: 'throughput_rps',
    }
    return map[m] ?? m
  }

  function handleRun() {
    if (!selectedCollection) return
    timeSeries = []
    const thresholds = thresholdsRaw
      .split('\n')
      .map((s) => s.trim())
      .filter((s) => s.length > 0)
    onRun({
      collection: selectedCollection,
      vus,
      duration_secs: durationSecs,
      load_profile: loadProfile,
      initial_vus: loadProfile === 'ramp_up' ? Math.min(initialVus, vus) : undefined,
      thresholds,
      data: dataFile.trim() || undefined,
    })
  }
</script>

{#if !started && !running}
  <!-- ── Setup form ──────────────────────────────────────────────────────── -->
  <div class="setup-view">
    <section class="setup-section">
      <h2 class="section-heading">Set up your performance test</h2>

      <label class="field-stack">
        <span class="field-label">Collection</span>
        {#if collections.length > 0}
          <select bind:value={selectedCollection} class="input-control">
            {#each collections as file}
              <option value={file}>{file}</option>
            {/each}
          </select>
        {:else}
          <p class="hint-text">No .yaml files found in the current directory.</p>
        {/if}
      </label>

      <div class="config-row">
        <label class="field-stack">
          <span class="field-label">
            Load profile
            <span class="info-icon" title="How virtual users are spawned over time">ⓘ</span>
          </span>
          <select bind:value={loadProfile} class="input-control">
            <option value="fixed">Fixed</option>
            <option value="ramp_up">Ramp up</option>
          </select>
        </label>

        <label class="field-stack">
          <span class="field-label">
            Virtual users
            <span class="info-icon" title="Number of concurrent simulated users">ⓘ</span>
          </span>
          <input type="number" min="1" max="500" bind:value={vus} class="input-control" />
        </label>

        <label class="field-stack">
          <span class="field-label">Test duration</span>
          <div class="input-with-unit">
            <input type="number" min="1" max="60" bind:value={durationMins} class="input-control" />
            <span class="unit-label">mins</span>
          </div>
        </label>
      </div>

      <!-- Load profile preview -->
      <div class="profile-preview">
        <svg
          viewBox="0 0 {PV_W} {PV_H}"
          width="100%"
          preserveAspectRatio="xMidYMid meet"
          aria-hidden="true"
        >
          <!-- Grid lines -->
          {#each [0.25, 0.5, 0.75] as frac}
            <line
              x1={PL}
              y1={PT + pH * (1 - frac)}
              x2={PL + pW}
              y2={PT + pH * (1 - frac)}
              stroke="#2a2a4a"
              stroke-width="1"
              stroke-dasharray="4,3"
            />
          {/each}
          <!-- Fill area -->
          <polygon points={profilePoints} fill="#ff6b3522" />
          <!-- Profile line -->
          <polyline points={profileLinePts} fill="none" stroke="#ff6b35" stroke-width="2" />
          <!-- Y axis -->
          <line x1={PL} y1={PT} x2={PL} y2={PT + pH} stroke="#333" stroke-width="1" />
          <!-- X axis -->
          <line x1={PL} y1={PT + pH} x2={PL + pW} y2={PT + pH} stroke="#333" stroke-width="1" />
          <!-- Y label: vus count -->
          <text x={PL - 6} y={PT + 4} text-anchor="end" fill="#888" font-size="10">{vus}</text>
          <text x={PL - 6} y={PT + pH} text-anchor="end" fill="#888" font-size="10">0</text>
          <!-- X labels -->
          <text x={PL} y={PT + pH + 16} fill="#888" font-size="10">0</text>
          <text x={PL + pW} y={PT + pH + 16} text-anchor="end" fill="#888" font-size="10"
            >{durationMins} min{durationMins === 1 ? '' : 's'}</text
          >
          {#if loadProfile === 'ramp_up'}
            <!-- Midpoint marker -->
            <line
              x1={PL + pW / 2}
              y1={PT}
              x2={PL + pW / 2}
              y2={PT + pH}
              stroke="#4a9eff"
              stroke-width="1"
              stroke-dasharray="4,3"
            />
            <text
              x={PL + pW / 2}
              y={PT + pH + 16}
              text-anchor="middle"
              fill="#4a9eff"
              font-size="10">{durationMins / 2} mins</text
            >
          {/if}
        </svg>
        <p class="profile-desc">{profileDesc}</p>
      </div>

      {#if loadProfile === 'ramp_up'}
        <label class="field-stack" style="max-width: 200px;">
          <span class="field-label">
            Initial load
            <span class="info-icon" title="VUs to start with before the ramp">ⓘ</span>
          </span>
          <input
            type="number"
            min="1"
            max={vus}
            bind:value={initialVus}
            class="input-control"
          />
        </label>
      {/if}

      <label class="field-stack">
        <span class="field-label">
          Data file
          <span class="info-icon" title="Optional CSV or JSON file for data-driven testing">ⓘ</span>
        </span>
        <input
          type="text"
          placeholder="path/to/data.csv or data.json"
          bind:value={dataFile}
          class="input-control"
        />
      </label>

      <div class="threshold-section">
        <button
          class="expand-btn"
          onclick={() => (showThresholds = !showThresholds)}
          aria-expanded={showThresholds}
        >
          <span class="expand-arrow" class:open={showThresholds}>›</span>
          Pass test if…
          <span class="info-icon" title="Define pass/fail criteria on metrics">ⓘ</span>
        </button>
        {#if showThresholds}
          <div class="threshold-form">
            <p class="hint-text">
              One per line: <code>METRIC:CONDITION:VALUE</code>
              <br />
              Metrics: <code>avg_response_ms</code>, <code>p95_response_ms</code>,
              <code>p99_response_ms</code>, <code>error_rate_pct</code>,
              <code>throughput_rps</code>
              <br />
              Conditions: <code>lt</code>, <code>lte</code>, <code>gt</code>, <code>gte</code>
            </p>
            <textarea
              rows="4"
              placeholder="p95_response_ms:lt:500&#10;error_rate_pct:lt:1"
              bind:value={thresholdsRaw}
              class="input-control"
            ></textarea>
          </div>
        {/if}
      </div>
    </section>

    <div class="run-bar">
      <button class="run-btn" onclick={handleRun} disabled={!selectedCollection}>
        Run
      </button>
    </div>
  </div>
{:else}
  <!-- ── Results view ────────────────────────────────────────────────────── -->
  <div class="results-view">
    <!-- Run header -->
    <header class="run-header">
      <div class="run-meta">
        {#if started}
          <span class="meta-collection">{selectedCollection || '—'}</span>
          <span class="meta-sep">·</span>
          <span class="meta-item">{started.vus} VU{started.vus === 1 ? '' : 's'}</span>
          <span class="meta-sep">·</span>
          <span class="meta-item">{Math.round(started.duration_secs / 60)} min{Math.round(started.duration_secs / 60) === 1 ? '' : 's'}</span>
          <span class="meta-sep">·</span>
          <span class="meta-item">{started.load_profile === 'ramp_up' ? 'Ramp up' : 'Fixed'} profile</span>
        {/if}
      </div>
      <div class="run-controls">
        {#if running}
          <span class="status-badge in-progress">
            <span class="pulse-dot"></span>In Progress
          </span>
          <span class="time-remaining">{timeRemainingStr}</span>
          <button class="stop-btn" onclick={onStop}>Stop run</button>
        {:else if passed === true}
          <span class="status-badge passed">✓ Passed</span>
          <button class="new-run-btn" onclick={onNewRun}>New run</button>
        {:else if passed === false}
          <span class="status-badge failed">✗ Failed</span>
          <button class="new-run-btn" onclick={onNewRun}>New run</button>
        {/if}
      </div>
    </header>

    <!-- Progress bar (during run) -->
    {#if running}
      <div class="progress-bar-track">
        <div class="progress-bar-fill" style:width="{elapsedPct}%"></div>
      </div>
    {/if}

    {#if error}
      <div class="error-box">{error}</div>
    {/if}

    <!-- Metrics strip -->
    {#if displayMetrics}
      <div class="metrics-strip">
        <div class="metric-item">
          <span class="metric-label">Total requests</span>
          <span class="metric-value">{displayMetrics.total_iterations.toLocaleString()}</span>
        </div>
        <div class="metric-divider"></div>
        <div class="metric-item">
          <span class="metric-label">Requests/second</span>
          <span class="metric-value">{fmtNum(displayMetrics.throughput_rps)}</span>
        </div>
        <div class="metric-divider"></div>
        <div class="metric-item">
          <span class="metric-label">Avg. response time</span>
          <span class="metric-value">{fmtMs(displayMetrics.avg_response_ms)} <span class="metric-unit">ms</span></span>
        </div>
        <div class="metric-divider"></div>
        <div class="metric-item">
          <span class="metric-label">P95</span>
          <span class="metric-value">{fmtMs(displayMetrics.p95_response_ms)} <span class="metric-unit">ms</span></span>
        </div>
        {#if finalMetrics}
          <div class="metric-divider"></div>
          <div class="metric-item">
            <span class="metric-label">P99</span>
            <span class="metric-value">{fmtMs(finalMetrics.p99_response_ms)} <span class="metric-unit">ms</span></span>
          </div>
        {/if}
        <div class="metric-divider"></div>
        <div class="metric-item">
          <span class="metric-label">Error %</span>
          <span class="metric-value" class:metric-error={displayMetrics.error_rate_pct > 0}>
            {displayMetrics.error_rate_pct.toFixed(2)}
          </span>
        </div>
        <div class="metric-divider"></div>
        <div class="metric-item">
          <span class="metric-label">Failure %</span>
          <span class="metric-value" class:metric-error={displayMetrics.failed_iterations > 0}>
            {displayMetrics.total_iterations > 0 ? ((displayMetrics.failed_iterations / displayMetrics.total_iterations) * 100).toFixed(2) : '0.00'}
          </span>
        </div>
      </div>

      <!-- Live chart -->
      {#if timeSeries.length > 1}
        <div class="chart-container">
          <svg
            viewBox="0 0 {CW} {CH}"
            width="100%"
            preserveAspectRatio="xMidYMid meet"
            aria-label="Performance metrics over time"
          >
            <!-- Horizontal grid lines -->
            {#each [0.25, 0.5, 0.75, 1] as frac}
              <line
                x1={ML}
                y1={MT + cH * (1 - frac)}
                x2={ML + cW}
                y2={MT + cH * (1 - frac)}
                stroke="#1e1e38"
                stroke-width="1"
              />
            {/each}

            <!-- VU reference line (gray) -->
            {#if vuLinePts && started}
              <polyline
                points={vuLinePts}
                fill="none"
                stroke="#555"
                stroke-width="1.5"
                stroke-dasharray="4 2"
                stroke-linejoin="round"
                stroke-linecap="round"
              />
            {/if}

            <!-- Throughput line (gold) -->
            {#if throughputPts}
              <polyline
                points={throughputPts}
                fill="none"
                stroke="#f59e0b"
                stroke-width="2"
                stroke-linejoin="round"
                stroke-linecap="round"
              />
            {/if}
            <!-- Avg response line (blue) -->
            {#if responsePts}
              <polyline
                points={responsePts}
                fill="none"
                stroke="#60a5fa"
                stroke-width="2"
                stroke-linejoin="round"
                stroke-linecap="round"
              />
            {/if}
            <!-- Error rate line (red) -->
            {#if errorPts}
              <polyline
                points={errorPts}
                fill="none"
                stroke="#f87171"
                stroke-width="2"
                stroke-linejoin="round"
                stroke-linecap="round"
              />
            {/if}

            <!-- Left Y-axis labels (ms/%) -->
            <text x={ML - 4} y={MT + 5} fill="#666" font-size="10" text-anchor="end">
              {maxResponse.toFixed(0)}ms
            </text>
            <text x={ML - 4} y={MT + cH + 4} fill="#666" font-size="10" text-anchor="end">0</text>
            
            <!-- Right Y-axis labels (req/s) -->
            <text x={ML + cW + 4} y={MT + 5} fill="#666" font-size="10" text-anchor="start">
              {maxThroughput.toFixed(1)} req/s
            </text>
            <text x={ML + cW + 4} y={MT + cH + 4} fill="#666" font-size="10" text-anchor="start">0</text>

            <!-- X axis -->
            <line x1={ML} y1={MT + cH} x2={ML + cW} y2={MT + cH} stroke="#2a2a4a" stroke-width="1" />
            <!-- X axis labels -->
            <text x={ML} y={MT + cH + 14} fill="#666" font-size="11">0s</text>
            <text x={ML + cW} y={MT + cH + 14} text-anchor="end" fill="#666" font-size="11">
              {started?.duration_secs ?? 0}s
            </text>
            <!-- Legend -->
            <g transform="translate({ML}, {MT + cH + 28})">
              <rect x="0" y="-5" width="16" height="2" fill="#f59e0b" />
              <text x="20" y="0" fill="#aaa" font-size="11">Requests/second</text>
              <rect x="150" y="-5" width="16" height="2" fill="#60a5fa" />
              <text x="170" y="0" fill="#aaa" font-size="11">Avg. response (ms)</text>
              <rect x="320" y="-5" width="16" height="2" fill="#f87171" />
              <text x="340" y="0" fill="#aaa" font-size="11">Error %</text>
              {#if started}
                <line x1="460" y1="-4" x2="476" y2="-4" stroke="#555" stroke-width="1.5" stroke-dasharray="4 2" />
                <text x="480" y="0" fill="#aaa" font-size="11">Virtual users</text>
              {/if}
            </g>
          </svg>
        </div>
      {:else if running}
        <div class="chart-placeholder">Collecting data…</div>
      {/if}

      <!-- Per-request breakdown table -->
      {#if (tick && tick.per_request.length > 0) || (finalMetrics && finalMetrics.per_request.length > 0)}
        <div class="per-request-section">
          <h3 class="sub-heading">Performance details for total duration</h3>
          <div class="table-wrapper">
            <table class="per-request-table">
              <thead>
                <tr>
                  <th>#</th>
                  <th>Request</th>
                  <th>Total</th>
                  <th>Req/s</th>
                  <th>Avg ms</th>
                  <th>Min</th>
                  <th>Max</th>
                  <th>P95</th>
                  <th>P99</th>
                  <th>Error %</th>
                  <th>Failure %</th>
                </tr>
              </thead>
              <tbody>
                {#if finalMetrics && finalMetrics.per_request.length > 0}
                  {#each finalMetrics.per_request as req, i}
                    <tr>
                      <td>{i + 1}</td>
                      <td class="req-name">{req.name}</td>
                      <td>{req.total.toLocaleString()}</td>
                      <td>{req.throughput_rps.toFixed(2)}</td>
                      <td>{req.avg_response_ms.toFixed(2)}</td>
                      <td>{req.min_response_ms.toFixed(2)}</td>
                      <td>{req.max_response_ms.toFixed(2)}</td>
                      <td>{req.p95_response_ms.toFixed(2)}</td>
                      <td>{req.p99_response_ms.toFixed(2)}</td>
                      <td class:metric-error={req.error_rate_pct > 0}>{req.error_rate_pct.toFixed(2)}</td>
                      <td class:metric-error={req.failed > 0}>{req.total > 0 ? ((req.failed / req.total) * 100).toFixed(2) : '0.00'}</td>
                    </tr>
                  {/each}
                {:else if tick && tick.per_request.length > 0}
                  {#each tick.per_request as req, i}
                    <tr>
                      <td>{i + 1}</td>
                      <td class="req-name">{req.name}</td>
                      <td>{req.total.toLocaleString()}</td>
                      <td>{req.throughput_rps.toFixed(2)}</td>
                      <td>{req.avg_response_ms.toFixed(2)}</td>
                      <td>—</td>
                      <td>—</td>
                      <td>—</td>
                      <td>—</td>
                      <td class:metric-error={req.error_rate_pct > 0}>{req.error_rate_pct.toFixed(2)}</td>
                      <td class:metric-error={req.failed > 0}>{req.total > 0 ? ((req.failed / req.total) * 100).toFixed(2) : '0.00'}</td>
                    </tr>
                  {/each}
                {/if}
              </tbody>
            </table>
          </div>
        </div>
      {/if}
    {/if}

    <!-- Threshold results -->
    {#if thresholdResults.length > 0}
      <div class="thresholds-section">
        <h3 class="sub-heading">Pass test if…</h3>
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
                <td>{tr.observed.toFixed(2)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
{/if}

<style>
  /* ── Shared ─────────────────────────────────────────────────────────────── */
  :global(.perf-area) {
    flex: 1;
    overflow-y: auto;
    color: #e0e0e0;
    background: #13132b;
  }

  .hint-text {
    margin: 0;
    font-size: 0.8rem;
    color: #666;
    line-height: 1.5;
  }

  .hint-text code {
    color: #aaa;
    background: #1a1a38;
    padding: 1px 4px;
    border-radius: 3px;
    font-size: 0.75rem;
  }

  .info-icon {
    color: #555;
    font-size: 0.75rem;
    cursor: help;
  }

  /* ── Setup view ──────────────────────────────────────────────────────────── */
  .setup-view {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  .setup-section {
    flex: 1;
    overflow-y: auto;
    padding: 32px 40px;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .section-heading {
    margin: 0;
    font-size: 1.1rem;
    font-weight: 600;
    color: #e0e0e0;
  }

  .config-row {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
  }

  .config-row .field-stack {
    flex: 1;
    min-width: 140px;
  }

  .field-stack {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .field-label {
    font-size: 0.82rem;
    color: #bbb;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .input-control {
    background: #0f0f23;
    border: 1px solid #2a2a4a;
    border-radius: 6px;
    color: #e0e0e0;
    padding: 9px 12px;
    font-size: 0.875rem;
    width: 100%;
    box-sizing: border-box;
    outline: none;
    transition: border-color 0.15s;
    resize: vertical;
  }

  .input-control:focus {
    border-color: #ff6b35;
  }

  .input-with-unit {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .input-with-unit .input-control {
    flex: 1;
  }

  .unit-label {
    color: #666;
    font-size: 0.82rem;
    white-space: nowrap;
  }

  /* Profile preview */
  .profile-preview {
    background: #0f0f23;
    border: 1px solid #2a2a4a;
    border-radius: 8px;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .profile-desc {
    margin: 0;
    font-size: 0.82rem;
    color: #888;
    line-height: 1.5;
  }

  /* Threshold collapsible */
  .threshold-section {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .expand-btn {
    background: none;
    border: none;
    color: #ccc;
    cursor: pointer;
    padding: 0;
    font-size: 0.9rem;
    display: flex;
    align-items: center;
    gap: 6px;
    text-align: left;
  }

  .expand-btn:hover {
    color: #fff;
  }

  .expand-arrow {
    display: inline-block;
    font-size: 1.1rem;
    transition: transform 0.15s;
    color: #888;
  }

  .expand-arrow.open {
    transform: rotate(90deg);
  }

  .threshold-form {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-left: 18px;
    border-left: 2px solid #2a2a4a;
  }

  /* Run bar */
  .run-bar {
    padding: 16px 40px;
    border-top: 1px solid #1e1e38;
    background: #13132b;
  }

  .run-btn {
    padding: 10px 28px;
    background: #ff6b35;
    color: white;
    border: none;
    border-radius: 6px;
    font-size: 0.95rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
  }

  .run-btn:hover:not(:disabled) {
    background: #ff8555;
  }

  .run-btn:disabled {
    background: #444;
    cursor: not-allowed;
  }

  /* ── Results view ────────────────────────────────────────────────────────── */
  .results-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow-y: auto;
  }

  .run-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 28px;
    border-bottom: 1px solid #1e1e38;
    background: #13132b;
    flex-wrap: wrap;
    gap: 10px;
  }

  .run-meta {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
    font-size: 0.85rem;
    color: #999;
  }

  .meta-collection {
    color: #e0e0e0;
    font-weight: 600;
  }

  .meta-sep {
    color: #444;
  }

  .run-controls {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .status-badge {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border-radius: 4px;
    font-size: 0.8rem;
    font-weight: 600;
  }

  .status-badge.in-progress {
    background: #1a2a3a;
    color: #60a5fa;
  }

  .status-badge.passed {
    background: #1a3a2a;
    color: #4ade80;
  }

  .status-badge.failed {
    background: #3a1a1a;
    color: #f87171;
  }

  .pulse-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #60a5fa;
    animation: pulse 1.4s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.3; }
  }

  .time-remaining {
    font-size: 0.82rem;
    color: #888;
    font-variant-numeric: tabular-nums;
  }

  .stop-btn {
    padding: 6px 14px;
    background: #f87171;
    color: white;
    border: none;
    border-radius: 5px;
    font-size: 0.82rem;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s;
  }

  .stop-btn:hover {
    background: #ef4444;
  }

  .new-run-btn {
    padding: 6px 14px;
    background: transparent;
    color: #aaa;
    border: 1px solid #333;
    border-radius: 5px;
    font-size: 0.82rem;
    cursor: pointer;
    transition: border-color 0.15s, color 0.15s;
  }

  .new-run-btn:hover {
    border-color: #ff6b35;
    color: #ff6b35;
  }

  /* Progress bar */
  .progress-bar-track {
    height: 3px;
    background: #1e1e38;
    position: relative;
  }

  .progress-bar-fill {
    height: 100%;
    background: #ff6b35;
    transition: width 0.5s linear;
  }

  /* Error box */
  .error-box {
    margin: 16px 28px;
    background: #3a1a1a;
    border: 1px solid #f87171;
    border-radius: 6px;
    padding: 12px 16px;
    color: #f87171;
    font-size: 0.875rem;
  }

  /* Metrics strip */
  .metrics-strip {
    display: flex;
    align-items: stretch;
    padding: 20px 28px;
    gap: 0;
    border-bottom: 1px solid #1e1e38;
    overflow-x: auto;
  }

  .metric-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 0 20px;
    flex-shrink: 0;
  }

  .metric-item:first-child {
    padding-left: 0;
  }

  .metric-label {
    font-size: 0.72rem;
    color: #888;
    white-space: nowrap;
  }

  .metric-value {
    font-size: 1.5rem;
    font-weight: 700;
    color: #e0e0e0;
    font-variant-numeric: tabular-nums;
    line-height: 1.1;
  }

  .metric-unit {
    font-size: 0.9rem;
    font-weight: 400;
    color: #888;
  }

  .metric-error {
    color: #f87171;
  }

  .metric-divider {
    width: 1px;
    background: #1e1e38;
    flex-shrink: 0;
    margin: 4px 0;
  }

  /* Chart */
  .chart-container {
    margin: 16px 28px;
    background: #0f0f23;
    border: 1px solid #1e1e38;
    border-radius: 8px;
    padding: 12px;
  }

  .chart-placeholder {
    margin: 24px 28px;
    color: #555;
    font-size: 0.85rem;
    font-style: italic;
  }

  /* Thresholds */
  .thresholds-section {
    margin: 0 28px 24px;
  }

  .sub-heading {
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
    color: #555;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    border-bottom: 1px solid #1e1e38;
  }

  .threshold-table td {
    padding: 10px 12px;
    border-bottom: 1px solid #13132b;
    font-variant-numeric: tabular-nums;
  }

  .row-pass {
    color: #4ade80;
  }

  .row-fail {
    color: #f87171;
  }

  .icon-cell {
    width: 24px;
    font-size: 1rem;
  }

  .per-request-section {
    margin: 24px 28px;
  }

  .table-wrapper {
    overflow-x: auto;
    max-height: 400px;
    overflow-y: auto;
    border: 1px solid #1e1e38;
    border-radius: 8px;
  }

  .per-request-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.875rem;
  }

  .per-request-table th {
    text-align: left;
    padding: 8px 12px;
    color: #555;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    border-bottom: 1px solid #1e1e38;
    background: #0f0f23;
    position: sticky;
    top: 0;
    z-index: 1;
  }

  .per-request-table td {
    padding: 10px 12px;
    border-bottom: 1px solid #13132b;
    font-variant-numeric: tabular-nums;
  }

  .per-request-table .req-name {
    color: #e0e0e0;
    font-weight: 500;
  }

  .per-request-table tbody tr:hover {
    background: #13132b;
  }

</style>
