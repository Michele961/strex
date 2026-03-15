// Implemented in Task 6
use crate::error::{AssertionFailure, RequestError};
use crate::http::HttpResponse;

/// Aggregated result of running all requests in a collection.
#[derive(Debug)]
pub struct CollectionResult {
    /// Results for each request in the collection, in declaration order.
    pub request_results: Vec<RequestResult>,
}

impl CollectionResult {
    /// Returns `true` iff every request outcome is [`RequestOutcome::Passed`].
    pub fn passed(&self) -> bool {
        todo!()
    }

    /// Count of requests whose outcome is not `Passed`.
    pub fn failure_count(&self) -> usize {
        todo!()
    }
}

/// Result of executing a single request through all applicable lifecycle phases.
#[derive(Debug)]
pub struct RequestResult {
    /// The request name from the collection YAML.
    pub name: String,
    /// Final outcome for this request.
    pub outcome: RequestOutcome,
    /// Full lifecycle duration (phase 1 start → phase 7 end), in milliseconds.
    pub duration_ms: u64,
    /// HTTP response captured in phase 4. `None` if a stopping error occurred before phase 3.
    pub response: Option<HttpResponse>,
}

/// Outcome of a single request execution.
#[derive(Debug)]
pub enum RequestOutcome {
    /// All assertions passed (or no assertions defined).
    Passed,
    /// One or more declarative assertions failed; all collected (execution continues).
    AssertionsFailed(Vec<AssertionFailure>),
    /// A stopping error occurred in phase 1 or 3; subsequent phases were skipped.
    Error(RequestError),
}

/// Run all requests in `collection` sequentially.
///
/// All per-request failures are captured in [`RequestOutcome`] — this function never fails.
pub async fn execute_collection(
    _collection: &crate::collection::Collection,
    _context: crate::context::ExecutionContext,
) -> CollectionResult {
    todo!()
}
