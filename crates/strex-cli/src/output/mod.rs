pub mod console;
pub mod json;
pub mod junit;

use strex_core::{Collection, CollectionResult, DataRunResult};

/// Aggregated result passed to all output formatters.
#[allow(dead_code)]
pub struct RunResult {
    pub collection: Collection,
    pub outcome: RunOutcome,
}

/// The execution outcome variant.
#[allow(dead_code)]
pub enum RunOutcome {
    Single(CollectionResult),
    DataDriven(DataRunResult),
}
