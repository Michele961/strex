import type { RequestSequenceItem, RunSummary } from './types'

export async function fetchCollections(): Promise<string[]> {
  const res = await fetch('/api/collections')
  if (!res.ok) throw new Error(`Failed to fetch collections: ${res.status}`)
  return res.json()
}

export async function fetchCollectionRequests(file: string): Promise<RequestSequenceItem[]> {
  const res = await fetch(`/api/collection-requests?file=${encodeURIComponent(file)}`)
  if (!res.ok) throw new Error(`Failed to fetch collection requests: ${res.status}`)
  return res.json()
}

export async function fetchDataPreview(file: string): Promise<Record<string, string>[]> {
  const res = await fetch(`/api/data-preview?file=${encodeURIComponent(file)}`)
  if (!res.ok) throw new Error(`Failed to fetch data preview: ${res.status}`)
  return res.json()
}

export async function saveHistory(payload: object): Promise<{ id: string }> {
  const res = await fetch('/api/history', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  })
  if (!res.ok) throw new Error(`Failed to save history: ${res.status}`)
  return res.json()
}

export async function fetchHistory(): Promise<RunSummary[]> {
  const res = await fetch('/api/history')
  if (!res.ok) throw new Error(`Failed to fetch history: ${res.status}`)
  return res.json()
}

export async function loadHistoryRun(id: string): Promise<unknown> {
  const res = await fetch(`/api/history/${encodeURIComponent(id)}`)
  if (!res.ok) throw new Error(`Failed to load history run: ${res.status}`)
  return res.json()
}

export async function importGenerate(payload: {
  source: 'curl' | 'openapi'
  input: string
  mode: 'scaffold' | 'with_tests'
}): Promise<{ yaml: string }> {
  const res = await fetch('/api/import/generate', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  })
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }))
    throw new Error(body.error ?? `Import failed: ${res.status}`)
  }
  return res.json()
}

export async function importSave(payload: {
  yaml: string
  filename: string
}): Promise<{ filename: string }> {
  const res = await fetch('/api/import/save', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  })
  if (!res.ok) {
    const body = await res.json().catch(() => ({ error: res.statusText }))
    throw new Error(body.error ?? `Save failed: ${res.status}`)
  }
  return res.json()
}
