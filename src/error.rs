use std::fmt;

use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProcessExit {
    Success,
    Failure,
    Blocked,
}

impl ProcessExit {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::Failure => 1,
            Self::Blocked => 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandError {
    exit_status: ProcessExit,
    code: &'static str,
    message: String,
    hint: String,
    details: Option<Value>,
}

impl CommandError {
    pub fn failure(
        code: &'static str,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            exit_status: ProcessExit::Failure,
            code,
            message: message.into(),
            hint: hint.into(),
            details: None,
        }
    }

    pub fn blocked(
        code: &'static str,
        message: impl Into<String>,
        hint: impl Into<String>,
    ) -> Self {
        Self {
            exit_status: ProcessExit::Blocked,
            code,
            message: message.into(),
            hint: hint.into(),
            details: None,
        }
    }

    #[must_use]
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    #[must_use]
    pub const fn exit_status(&self) -> ProcessExit {
        self.exit_status
    }

    #[must_use]
    pub const fn code(&self) -> &str {
        self.code
    }

    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    #[must_use]
    pub fn hint(&self) -> &str {
        &self.hint
    }

    #[must_use]
    pub const fn details(&self) -> Option<&Value> {
        self.details.as_ref()
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CommandError {}
