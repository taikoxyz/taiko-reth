use clap::{
    builder::{NonEmptyStringValueParser, TypedValueParser},
    Arg, Command, Error,
};
use serde::{
    de::{self, DeserializeOwned, Visitor},
    Serialize,
};
use std::{path::PathBuf, str::FromStr};

/// InputSource represents both stdin and file input
#[derive(Debug, Clone)]
pub enum InputSource {
    Stdin,
    File(PathBuf),
}

impl InputSource {
    pub fn from_json<A, B>(&self) -> eyre::Result<B>
    where
        A: DeserializeOwned + From<B>,
        B: DeserializeOwned,
    {
        match self {
            InputSource::Stdin => serde_json::from_reader::<_, A>(std::io::stdin()),
            InputSource::File(path) => {
                serde_json::from_reader::<_, B>(std::fs::File::open(path)?).map(Into::into)
            }
        }
        .map_err(Into::into)
    }
}

/// Clap value parser for [InputSource]s that takes either a specify "stdin" or the path
/// to the InputSource.
pub fn input_source_value_parser(s: &str) -> eyre::Result<InputSource, eyre::Error> {
    Ok(match s {
        "stdin" => InputSource::Stdin,
        x => InputSource::File(PathBuf::from(x)),
    })
}

/// OutputTarget represents both stdout, stderr and file output
#[derive(Debug, Clone)]
pub enum OutputTarget {
    Stdout,
    Stderr,
    File(PathBuf),
}

impl OutputTarget {
    /// write buffer to output target
    pub fn to_json<T>(&self, value: &T) -> eyre::Result<()>
    where
        T: Serialize + ?Sized,
    {
        match self {
            OutputTarget::Stdout => serde_json::to_writer(std::io::stdout(), value),
            OutputTarget::Stderr => serde_json::to_writer(std::io::stderr(), value),
            OutputTarget::File(path) => serde_json::to_writer(std::fs::File::open(path)?, value),
        }
        .map_err(Into::into)
    }
}

/// Clap value parser for [OutputTarget]s that takes either a specify "stdout", "stderr" or the path
/// to the OutputSource.
pub fn output_source_value_parser(s: &str) -> eyre::Result<OutputTarget, eyre::Error> {
    Ok(match s {
        "stdout" => OutputTarget::Stdout,
        "stderr" => OutputTarget::Stderr,
        x => OutputTarget::File(PathBuf::from(x)),
    })
}
