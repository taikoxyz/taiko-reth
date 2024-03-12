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

/// A util for clap value_parser
pub struct InputSourceValueParser;

impl TypedValueParser for InputSourceValueParser {
    type Value = InputSource;

    fn parse_ref(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, Error> {
        let value = NonEmptyStringValueParser::new().parse_ref(cmd, arg, value)?;
        Ok(match value.as_str() {
            "stdin" => InputSource::Stdin,
            x => InputSource::File(PathBuf::from(x)),
        })
    }
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

pub struct OutputTargetValueParser;

impl TypedValueParser for OutputTargetValueParser {
    type Value = OutputTarget;

    fn parse_ref(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &std::ffi::OsStr,
    ) -> Result<Self::Value, Error> {
        let value = NonEmptyStringValueParser::new().parse_ref(cmd, arg, value)?;
        Ok(match value.as_str() {
            "stdout" => OutputTarget::Stdout,
            "stderr" => OutputTarget::Stderr,
            x => OutputTarget::File(PathBuf::from(x)),
        })
    }
}
