//! Handler for the `strex ui` subcommand.

use strex_ui::{start_server, ServerOpts};

use crate::cli::UiArgs;

/// Execute the `ui` subcommand — start the web server and open the browser.
///
/// Blocks until the server is stopped (Ctrl+C).
pub async fn execute(args: UiArgs) -> anyhow::Result<i32> {
    let opts = ServerOpts {
        port: args.port,
        collection: args.collection,
    };
    start_server(opts).await?;
    Ok(0)
}
