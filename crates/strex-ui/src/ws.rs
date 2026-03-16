//! WebSocket run handler — streams execution events to the browser.

use axum::{extract::ws::WebSocketUpgrade, response::IntoResponse};

/// WebSocket upgrade handler — full implementation in Task 5.
pub async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|_socket| async {
        // Task 5: receive run config, execute collection, stream WsEvents
    })
}
