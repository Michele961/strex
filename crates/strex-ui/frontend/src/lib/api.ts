import type { RequestSequenceItem } from './types'

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
