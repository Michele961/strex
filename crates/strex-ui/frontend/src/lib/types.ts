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
