export async function fetchCollections(): Promise<string[]> {
  const res = await fetch('/api/collections')
  if (!res.ok) throw new Error(`Failed to fetch collections: ${res.status}`)
  return res.json()
}
