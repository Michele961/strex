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
      response_body: string | null
      response_headers: Record<string, string> | null
    }
  | {
      type: 'run_finished'
      passed: number
      failed: number
      total_duration_ms: number
      avg_response_ms: number
    }
  | { type: 'error'; message: string }

export interface RequestResult {
  name: string
  method: string
  passed: boolean
  status: number | null
  duration_ms: number
  failures: string[]
  error: string | null
  response_body: string | null
  response_headers: Record<string, string> | null
}

export interface RequestSequenceItem {
  name: string
  method: string
}
