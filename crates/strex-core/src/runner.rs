// Implemented in Task 6
use crate::error::{AssertionFailure, RequestError};
use crate::http::HttpResponse;

/// The aggregated result of running an entire collection.
#[derive(Debug)]
pub struct CollectionResult {
    pub request_results: Vec<RequestResult>,
}

impl CollectionResult {
    /// Returns `true` if all requests passed with no assertion failures or errors.
    pub fn passed(&self) -> bool {
        todo!()
    }

    /// Returns the number of requests that failed (assertion failures or errors).
    pub fn failure_count(&self) -> usize {
        todo!()
    }
}

/// The result of a single request execution within a collection run.
#[derive(Debug)]
pub struct RequestResult {
    pub name: String,
    pub outcome: RequestOutcome,
    pub duration_ms: u64,
    pub response: Option<HttpResponse>,
}

/// The outcome of a single request execution.
#[derive(Debug)]
pub enum RequestOutcome {
    Passed,
    AssertionsFailed(Vec<AssertionFailure>),
    Error(RequestError),
}

/// Execute all requests in a collection and return the aggregated result.
pub async fn execute_collection(
    _collection: &crate::collection::Collection,
    _context: crate::context::ExecutionContext,
) -> CollectionResult {
    todo!()
}
