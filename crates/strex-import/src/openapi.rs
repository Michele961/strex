use crate::{ImportError, ImportMode};

pub(crate) fn convert(_spec: &str, _mode: ImportMode) -> Result<String, ImportError> {
    Err(ImportError::OpenApiParse("not implemented".into()))
}
