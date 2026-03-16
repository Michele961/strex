use std::io::Write;

use strex_core::{CollectionResult, RequestOutcome};

use crate::output::{format_failure, RunOutcome, RunResult};

/// Escape XML special characters in a string.
///
/// Replaces `&`, `<`, `>`, `"`, and `'` with their XML entity equivalents.
fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            other => out.push(other),
        }
    }
    out
}

/// Write a single `<testsuite>` element to `writer`.
///
/// Each `<testcase>` corresponds to one [`RequestResult`] in `col_result`.
/// - Passed requests produce a self-closing `<testcase name="..."/>`.
/// - Assertion failures produce a `<testcase>` with a `<failure message="..."/>` child.
/// - Errors produce a `<testcase>` with an `<error message="..."/>` child.
fn write_testsuite(
    writer: &mut impl Write,
    suite_name: &str,
    col_result: &CollectionResult,
) -> anyhow::Result<()> {
    let tests = col_result.request_results.len();
    let failures = col_result
        .request_results
        .iter()
        .filter(|r| matches!(r.outcome, RequestOutcome::AssertionsFailed(_)))
        .count();
    let errors = col_result
        .request_results
        .iter()
        .filter(|r| matches!(r.outcome, RequestOutcome::Error(_)))
        .count();

    writeln!(
        writer,
        r#"  <testsuite name="{}" tests="{}" failures="{}" errors="{}">"#,
        escape_xml(suite_name),
        tests,
        failures,
        errors,
    )?;

    for req in &col_result.request_results {
        let escaped_name = escape_xml(&req.name);
        match &req.outcome {
            RequestOutcome::Passed => {
                writeln!(writer, r#"    <testcase name="{}"/>"#, escaped_name)?;
            }
            RequestOutcome::AssertionsFailed(assertion_failures) => {
                // Use the first failure's message; subsequent failures are omitted per spec.
                let msg = assertion_failures
                    .first()
                    .map(|f| escape_xml(&format_failure(f)))
                    .unwrap_or_default();
                writeln!(writer, r#"    <testcase name="{}">"#, escaped_name)?;
                writeln!(writer, r#"      <failure message="{}"/>"#, msg)?;
                writeln!(writer, r#"    </testcase>"#)?;
            }
            RequestOutcome::Error(e) => {
                let msg = escape_xml(&e.to_string());
                writeln!(writer, r#"    <testcase name="{}">"#, escaped_name)?;
                writeln!(writer, r#"      <error message="{}"/>"#, msg)?;
                writeln!(writer, r#"    </testcase>"#)?;
            }
        }
    }

    writeln!(writer, r#"  </testsuite>"#)?;
    Ok(())
}

/// Write a JUnit XML report to `writer`.
///
/// Single runs produce one `<testsuite>` named after the collection.
/// Data-driven runs produce one `<testsuite>` per iteration, named
/// `"<collection-name> row <row_index>"`.
///
/// The output starts with an XML declaration and wraps all suites in a
/// `<testsuites>` root element.
pub fn print(result: &RunResult, writer: &mut impl Write) -> anyhow::Result<()> {
    writeln!(writer, r#"<?xml version="1.0" encoding="UTF-8"?>"#)?;
    writeln!(writer, r#"<testsuites>"#)?;

    match &result.outcome {
        RunOutcome::Single(col_result) => {
            write_testsuite(writer, &result.collection.name, col_result)?;
        }
        RunOutcome::DataDriven(data_result) => {
            for iter in &data_result.iterations {
                let suite_name = format!("{} row {}", result.collection.name, iter.row_index);
                write_testsuite(writer, &suite_name, &iter.collection_result)?;
            }
        }
    }

    writeln!(writer, r#"</testsuites>"#)?;
    Ok(())
}

#[cfg(test)]
#[path = "junit_tests.rs"]
mod tests;
