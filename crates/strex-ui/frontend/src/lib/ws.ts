import type { RunConfig, WsEvent, PerfRunConfig, PerfWsEvent } from './types'

export function connectRun(
  config: RunConfig,
  onEvent: (event: WsEvent) => void,
  onClose: () => void
): WebSocket {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const ws = new WebSocket(`${protocol}//${window.location.host}/ws`)

  ws.onopen = () => {
    ws.send(JSON.stringify(config))
  }

  ws.onmessage = (msg) => {
    try {
      const event: WsEvent = JSON.parse(msg.data)
      onEvent(event)
    } catch {
      console.error('Failed to parse WebSocket message:', msg.data)
    }
  }

  ws.onclose = () => onClose()
  ws.onerror = (e) => console.error('WebSocket error:', e)

  return ws
}

export function connectPerf(
  config: PerfRunConfig,
  onEvent: (event: PerfWsEvent) => void,
  onClose: () => void
): WebSocket {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const ws = new WebSocket(`${protocol}//${window.location.host}/ws/perf`)

  ws.onopen = () => {
    ws.send(JSON.stringify(config))
  }

  ws.onmessage = (msg) => {
    try {
      const event: PerfWsEvent = JSON.parse(msg.data)
      console.log('[WS] Received perf event:', event)
      onEvent(event)
    } catch (e) {
      console.error('Failed to parse perf WebSocket message:', msg.data, e)
    }
  }

  ws.onclose = () => onClose()
  ws.onerror = (e) => console.error('Perf WebSocket error:', e)

  return ws
}
