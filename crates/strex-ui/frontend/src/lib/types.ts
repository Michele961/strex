export interface ConsoleLog {
  level: 'log' | 'warn' | 'error'
  message: string
}

export interface RunConfig {
  collection: string
  data?: string
  concurrency: number
  fail_fast: boolean
  max_iterations?: number
  repeat_iterations?: number
  delay_between_requests_ms?: number
  delay_between_iterations_ms?: number
}

export type WsEvent =
  | { type: 'run_started'; total: number }
  | { type: 'iteration_started'; iteration: number; row: Record<string, string> }
  | {
      type: 'request_completed'
      name: string
      method: string
      url: string
      passed: boolean
      status: number | null
      duration_ms: number
      failures: string[]
      passed_assertions: string[]
      error: string | null
      response_body: string | null
      response_headers: Record<string, string> | null
      request_body: string | null
      logs: ConsoleLog[]
    }
  | {
      type: 'run_finished'
      passed: number
      failed: number
      skipped: number
      total_duration_ms: number
      avg_response_ms: number
    }
  | { type: 'error'; message: string }

export type ResultItem =
  | { type: 'iteration'; iteration: number; row: Record<string, string> }
  | { type: 'request'; result: RequestResult }

export interface RequestResult {
  name: string
  method: string
  url: string
  passed: boolean
  status: number | null
  duration_ms: number
  failures: string[]
  passed_assertions: string[]
  error: string | null
  response_body: string | null
  response_headers: Record<string, string> | null
  request_body: string | null
  logs: ConsoleLog[]
}

export interface RequestSequenceItem {
  name: string
  method: string
}

export interface RunSummary {
  id: string
  timestamp: string
  collection: string
  passed: number
  failed: number
  skipped: number
}

// ── Performance testing ───────────────────────────────────────────────────────

export interface RequestTick {
  name: string
  total: number
  passed: number
  failed: number
  throughput_rps: number
  avg_response_ms: number
  error_rate_pct: number
}

export interface RequestMetrics {
  name: string
  total: number
  passed: number
  failed: number
  avg_response_ms: number
  min_response_ms: number
  max_response_ms: number
  p50_response_ms: number
  p95_response_ms: number
  p99_response_ms: number
  error_rate_pct: number
  throughput_rps: number
}

export interface PerfRunConfig {
  collection: string
  vus?: number
  duration_secs?: number
  load_profile?: 'fixed' | 'ramp_up'
  initial_vus?: number
  thresholds: string[]
  data?: string
}

export interface PerfTick {
  elapsed_secs: number
  total_iterations: number
  passed_iterations: number
  failed_iterations: number
  throughput_rps: number
  error_rate_pct: number
  avg_response_ms: number
  p95_response_ms: number
  per_request: RequestTick[]
}

export interface PerfMetrics {
  total_iterations: number
  passed_iterations: number
  failed_iterations: number
  avg_response_ms: number
  min_response_ms: number
  max_response_ms: number
  p50_response_ms: number
  p95_response_ms: number
  p99_response_ms: number
  error_rate_pct: number
  throughput_rps: number
  elapsed_secs: number
  per_request: RequestMetrics[]
}

export interface ThresholdResult {
  threshold: {
    metric: string
    condition: string
    value: number
  }
  observed: number
  passed: boolean
}

export interface ChartPoint {
  elapsed_secs: number
  throughput_rps: number
  avg_response_ms: number
  error_rate_pct: number
  p95_response_ms: number
}

export type PerfWsEvent =
  | { type: 'Started'; vus: number; duration_secs: number; load_profile: string }
  | {
      type: 'Tick'
      elapsed_secs: number
      total_iterations: number
      passed_iterations: number
      failed_iterations: number
      throughput_rps: number
      error_rate_pct: number
      avg_response_ms: number
      p95_response_ms: number
      per_request: RequestTick[]
    }
  | { type: 'Finished'; metrics: PerfMetrics; threshold_results: ThresholdResult[]; passed: boolean }
  | { type: 'error'; message: string }

export interface PerfRunSummary {
  id: string
  timestamp: string
  collection: string
  vus: number
  duration_secs: number
  load_profile: string
  total_iterations: number
  throughput_rps: number
  avg_response_ms: number
  p95_response_ms: number
  error_rate_pct: number
  passed: boolean
}
