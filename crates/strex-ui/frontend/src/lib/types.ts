export interface RunConfig {
  collection: string
  data?: string
  concurrency: number
  fail_fast: boolean
}

export type WsEvent =
  | { type: 'run_started'; total: number }
  | {
      type: 'request_completed'
      name: string
      method: string
      passed: boolean
      status: number | null
      duration_ms: number
      failures: string[]
      error: string | null
    }
  | { type: 'run_finished'; passed: number; failed: number }
  | { type: 'error'; message: string }

export interface RequestResult {
  name: string
  method: string
  passed: boolean
  status: number | null
  duration_ms: number
  failures: string[]
  error: string | null
}
