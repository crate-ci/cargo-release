#[derive(Debug)]
pub struct CliError {
    error: Option<anyhow::Error>,
    code: i32,
}

impl CliError {
    pub fn silent(code: i32) -> Self {
        Self { error: None, code }
    }

    pub fn message(e: impl Into<anyhow::Error>) -> Self {
        Self {
            error: Some(e.into()),
            code: 101,
        }
    }
}

macro_rules! process_error_from {
    ($from:ty) => {
        impl From<$from> for CliError {
            fn from(error: $from) -> Self {
                Self::message(error)
            }
        }
    };
}

process_error_from!(anyhow::Error);
process_error_from!(std::io::Error);
process_error_from!(semver::Error);
process_error_from!(ignore::Error);
process_error_from!(tame_index::Error);
process_error_from!(cargo_metadata::Error);
process_error_from!(toml::ser::Error);
process_error_from!(toml_edit::ser::Error);

impl From<i32> for CliError {
    fn from(code: i32) -> Self {
        Self::silent(code)
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(error) = self.error.as_ref() {
            error.fmt(f)
        } else {
            Ok(())
        }
    }
}

/// Report any error message and exit.
pub fn exit(result: Result<(), CliError>) -> ! {
    let code = report(result);
    std::process::exit(code)
}

/// Report, delegating exiting to the caller.
pub fn report(result: Result<(), CliError>) -> i32 {
    match result {
        Ok(()) => 0,
        Err(err) => {
            if let Some(error) = err.error {
                // At this point, we might be exiting due to a broken pipe, just do our best and
                // move on.
                let _ = crate::ops::shell::error(error);
            }
            err.code
        }
    }
}

pub type CargoResult<T> = anyhow::Result<T>;
