import type { RunConfig, WsEvent } from './types'

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
