use std::path::PathBuf;

/// Options for the strex UI server.
pub struct ServerOpts {
    /// TCP port to bind on. Default: 7878.
    pub port: u16,
    /// Optional collection path to pre-select in the UI.
    pub collection: Option<PathBuf>,
}

/// Start the Axum server and open the browser.
///
/// Binds to `127.0.0.1:<port>`, prints the URL, and opens the default browser.
/// Returns when the server shuts down.
pub async fn start_server(_opts: ServerOpts) -> anyhow::Result<()> {
    todo!()
}
