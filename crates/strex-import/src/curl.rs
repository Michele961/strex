use crate::{ImportError, ImportMode};

pub(crate) fn convert(_input: &str, _mode: ImportMode) -> Result<String, ImportError> {
    Err(ImportError::CurlParse("not implemented".into()))
}
