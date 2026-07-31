use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{code} at {path}: {message}")]
pub struct SpecError {
    code: &'static str,
    path: String,
    message: String,
}

impl SpecError {
    pub(crate) fn new(
        code: &'static str,
        path: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            code,
            path: path.into(),
            message: message.into(),
        }
    }

    #[must_use]
    pub fn code(&self) -> &'static str {
        self.code
    }

    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{code}: {message}")]
pub struct DriverError {
    code: String,
    message: String,
    retryable: bool,
}

impl DriverError {
    #[must_use]
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            retryable: false,
        }
    }

    #[must_use]
    pub fn with_retryable(mut self, retryable: bool) -> Self {
        self.retryable = retryable;
        self
    }

    #[must_use]
    pub fn code(&self) -> &str {
        &self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn retryable(&self) -> bool {
        self.retryable
    }
}
