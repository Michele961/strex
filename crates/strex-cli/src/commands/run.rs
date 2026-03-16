use strex_core::{
    parse_collection, parse_csv, parse_json, run_collection_with_data, DataRunOpts,
    ExecutionContext, RunnerOpts,
};

use crate::cli::RunArgs;
use crate::output::{self, RunOutcome, RunResult};

/// Execute the `run` subcommand.
///
/// Returns `Ok(0)` when all requests pass, `Ok(1)` when any request fails,
/// and `Err(...)` (which becomes exit code 2 in `main`) for infrastructure errors
/// such as missing files, unsupported data extensions, or I/O failures.
pub async fn execute(args: RunArgs) -> anyhow::Result<i32> {
    // Step 1 — Parse collection file.
    let collection = parse_collection(&args.collection)?;

    // Step 2 — Optionally load data rows from a CSV or JSON file.
    let data_rows = if let Some(ref data_path) = args.data {
        let content = std::fs::read_to_string(data_path)?;
        let ext = data_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let rows = match ext {
            "csv" => parse_csv(&content)?,
            "json" => parse_json(&content)?,
            other => anyhow::bail!("unsupported data file extension: {}", other),
        };
        Some(rows)
    } else {
        None
    };

    // Step 3 — Execute (data-driven or single run) and build RunResult.
    let result = if let Some(rows) = data_rows {
        let opts = DataRunOpts {
            concurrency: args.concurrency,
            fail_fast: args.fail_fast,
            runner_opts: RunnerOpts::default(),
        };
        // `run_collection_with_data` takes ownership of collection, so clone first.
        let data_result = run_collection_with_data(collection.clone(), rows, opts).await?;
        RunResult {
            collection,
            outcome: RunOutcome::DataDriven(data_result),
        }
    } else {
        let ctx = ExecutionContext::new(&collection);
        let col_result = strex_core::execute_collection(&collection, ctx).await;
        RunResult {
            collection,
            outcome: RunOutcome::Single(col_result),
        }
    };

    // Step 4 — Write formatted output to stdout or a file.
    let mut writer: Box<dyn std::io::Write> = match &args.output {
        Some(path) => Box::new(std::fs::File::create(path)?),
        None => Box::new(std::io::stdout()),
    };
    output::format(&result, &args.format, &mut writer)?;

    // Step 5 — Return exit code.
    if result.passed() {
        Ok(0)
    } else {
        Ok(1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::OutputFormat;
    use std::io::Write;
    use tempfile::NamedTempFile;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Write a minimal single-request collection YAML to a temp file.
    ///
    /// `url` is interpolated into the YAML so you can vary it per test.
    /// `assertion_status` sets the expected HTTP status code in the assertion.
    fn make_collection_file(url: &str, assertion_status: u16) -> NamedTempFile {
        let mut f = NamedTempFile::new().unwrap();
        write!(
            f,
            r#"name: Test Collection
version: "1.0"
requests:
  - name: Get Root
    method: GET
    url: "{url}/ok"
    assertions:
      - status: {assertion_status}
"#,
            url = url,
            assertion_status = assertion_status,
        )
        .unwrap();
        f
    }

    /// Build a `RunArgs` value pointing at `col_path` with sensible defaults.
    fn make_args(
        col_path: std::path::PathBuf,
        data: Option<std::path::PathBuf>,
        format: OutputFormat,
        output: Option<std::path::PathBuf>,
    ) -> RunArgs {
        RunArgs {
            collection: col_path,
            data,
            concurrency: 1,
            fail_fast: false,
            format,
            output,
        }
    }

    // -------------------------------------------------------------------------
    // Test 1 — single run, all passed → exit 0
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn single_run_all_passed_returns_exit_zero() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ok"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let col = make_collection_file(&server.uri(), 200);
        let args = make_args(col.path().to_path_buf(), None, OutputFormat::Console, None);
        let code = execute(args).await.unwrap();
        assert_eq!(code, 0);
    }

    // -------------------------------------------------------------------------
    // Test 2 — single run, assertion expects 200, server returns 404 → exit 1
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn single_run_with_failure_returns_exit_one() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ok"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let col = make_collection_file(&server.uri(), 200);
        let args = make_args(col.path().to_path_buf(), None, OutputFormat::Console, None);
        let code = execute(args).await.unwrap();
        assert_eq!(code, 1);
    }

    // -------------------------------------------------------------------------
    // Test 3 — data-driven path executes without error
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn data_driven_run_returns_ok() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ok"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let col = make_collection_file(&server.uri(), 200);

        // Two-row CSV (header + 2 data rows; collection URL has no placeholders).
        let mut csv_file = tempfile::Builder::new().suffix(".csv").tempfile().unwrap();
        write!(csv_file, "id\n1\n2\n").unwrap();

        let args = make_args(
            col.path().to_path_buf(),
            Some(csv_file.path().to_path_buf()),
            OutputFormat::Console,
            None,
        );
        let result = execute(args).await;
        assert!(result.is_ok(), "data-driven run must not return Err");
        // Both rows hit the same 200-returning mock → all pass → exit 0
        assert_eq!(result.unwrap(), 0);
    }

    // -------------------------------------------------------------------------
    // Test 4 — --output writes output to a file
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn output_written_to_file_when_output_flag_set() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ok"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let col = make_collection_file(&server.uri(), 200);
        let out_file = NamedTempFile::new().unwrap();
        let out_path = out_file.path().to_path_buf();

        let args = make_args(
            col.path().to_path_buf(),
            None,
            OutputFormat::Console,
            Some(out_path.clone()),
        );
        let code = execute(args).await.unwrap();
        assert_eq!(code, 0);

        let content = std::fs::read_to_string(&out_path).unwrap();
        // Console output must mention at least one request result.
        assert!(
            content.contains("Get Root"),
            "output file should contain request name; got: {content}"
        );
    }

    // -------------------------------------------------------------------------
    // Test 5 — unsupported data extension returns Err
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn unsupported_data_extension_returns_error() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ok"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let col = make_collection_file(&server.uri(), 200);

        // Write a .txt data file — unsupported extension.
        let mut txt_file = tempfile::Builder::new().suffix(".txt").tempfile().unwrap();
        write!(txt_file, "id\n1\n").unwrap();

        let args = make_args(
            col.path().to_path_buf(),
            Some(txt_file.path().to_path_buf()),
            OutputFormat::Console,
            None,
        );
        let result = execute(args).await;
        assert!(result.is_err(), "should return Err for .txt extension");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("unsupported data file extension"),
            "error message should mention unsupported extension; got: {err_msg}"
        );
    }

    // -------------------------------------------------------------------------
    // Test 6 — --format json output is valid JSON
    // -------------------------------------------------------------------------
    #[tokio::test]
    async fn json_format_output_is_valid_json() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/ok"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&server)
            .await;

        let col = make_collection_file(&server.uri(), 200);

        // Capture JSON output to a temp file so we can read it back.
        let out_file = NamedTempFile::new().unwrap();
        let out_path = out_file.path().to_path_buf();

        let args = make_args(
            col.path().to_path_buf(),
            None,
            OutputFormat::Json,
            Some(out_path.clone()),
        );
        execute(args).await.unwrap();

        let content = std::fs::read_to_string(&out_path).unwrap();
        let parsed: serde_json::Value =
            serde_json::from_str(&content).expect("output should be valid JSON");
        assert!(
            parsed.is_object(),
            "JSON output should be an object; got: {parsed}"
        );
    }
}
