use anyhow::anyhow;
use std::{error::Error, fmt};

#[derive(Debug)]
struct InputError(String);

impl fmt::Display for InputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl Error for InputError {}

pub(crate) fn input_error(message: impl Into<String>) -> anyhow::Error {
    anyhow!(InputError(message.into()))
}

pub(crate) fn is_input_error(error: &anyhow::Error) -> bool {
    error.downcast_ref::<InputError>().is_some()
}
