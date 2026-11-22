use std::fmt;
use std::path::PathBuf;

pub const DEFAULT_PORT: u16 = 3001;

/// Name of the environment variable that sets the port when `--port` is not given.
pub const PORT_VARIABLE: &str = "PORT";

/// Name of the environment variable that names the mocks file when `--mocks` is not given.
pub const MOCKS_VARIABLE: &str = "MOCKS";

pub const USAGE: &str = "\
Usage: mockiapi [--port <PORT>] [--mocks <FILE>]

Options:
  -p, --port <PORT>   Port to listen on. Defaults to the PORT environment variable, then 3001.
  -m, --mocks <FILE>  Endpoints to register at startup: a mockiapi file in JSON or YAML, or a
                      Postman collection. Defaults to the MOCKS environment variable.
  -h, --help          Print this help.";

/// What the command line asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Serve {
        port: u16,
        /// The file to register endpoints from before the server starts listening.
        mocks: Option<PathBuf>,
    },
    Help,
}

/// What the environment says, for the options the command line left out.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Environment {
    pub port: Option<String>,
    pub mocks: Option<String>,
}

impl Environment {
    pub fn read() -> Self {
        Self {
            port: std::env::var(PORT_VARIABLE).ok(),
            mocks: std::env::var(MOCKS_VARIABLE).ok(),
        }
    }
}

/// Why the server could not work out how to start.
#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// The value given for the port is not a number between 1 and 65535.
    InvalidPort(String),
    /// `--port` came last, with nothing after it.
    MissingPortValue,
    /// `--mocks` came last, with nothing after it.
    MissingMocksValue,
    UnknownArgument(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::InvalidPort(value) => {
                write!(formatter, "'{value}' is not a port: use a number between 1 and 65535")
            }
            ConfigError::MissingPortValue => write!(formatter, "--port needs a value, for example --port 8080"),
            ConfigError::MissingMocksValue => {
                write!(formatter, "--mocks needs a file, for example --mocks mocks.yaml")
            }
            ConfigError::UnknownArgument(argument) => write!(formatter, "unknown argument '{argument}'"),
        }
    }
}

/// Reads the command line, without the program name, and the environment.
///
/// An argument wins over its variable, and an empty variable counts as not set.
pub fn parse(arguments: impl IntoIterator<Item = String>, environment: Environment) -> Result<Command, ConfigError> {
    let mut arguments = arguments.into_iter();
    let mut requested_port = None;
    let mut requested_mocks = None;

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "-p" | "--port" => {
                requested_port = Some(arguments.next().ok_or(ConfigError::MissingPortValue)?);
            }
            "-m" | "--mocks" => {
                requested_mocks = Some(arguments.next().ok_or(ConfigError::MissingMocksValue)?);
            }
            other => {
                if let Some(value) = other.strip_prefix("--port=") {
                    requested_port = Some(value.to_string());
                } else if let Some(value) = other.strip_prefix("--mocks=") {
                    requested_mocks = Some(value.to_string());
                } else {
                    return Err(ConfigError::UnknownArgument(argument));
                }
            }
        }
    }

    let port = requested_port
        .or(environment.port.filter(|value| !value.trim().is_empty()))
        .map(|value| parse_port(&value))
        .transpose()?
        .unwrap_or(DEFAULT_PORT);
    let mocks = requested_mocks
        .or(environment.mocks)
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from);

    Ok(Command::Serve { port, mocks })
}

fn parse_port(value: &str) -> Result<u16, ConfigError> {
    value
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|port| *port != 0)
        .ok_or_else(|| ConfigError::InvalidPort(value.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(arguments: &[&str], port_variable: Option<&str>) -> Result<Command, ConfigError> {
        parsed_with(arguments, port_variable, None)
    }

    fn parsed_with(
        arguments: &[&str],
        port_variable: Option<&str>,
        mocks_variable: Option<&str>,
    ) -> Result<Command, ConfigError> {
        let arguments = arguments
            .iter()
            .map(|argument| argument.to_string());
        let environment = Environment {
            port: port_variable.map(str::to_string),
            mocks: mocks_variable.map(str::to_string),
        };

        parse(arguments, environment)
    }

    fn serve(port: u16) -> Result<Command, ConfigError> {
        Ok(Command::Serve { port, mocks: None })
    }

    fn serve_with_mocks(file: &str) -> Result<Command, ConfigError> {
        Ok(Command::Serve { port: DEFAULT_PORT, mocks: Some(PathBuf::from(file)) })
    }

    #[test]
    fn reads_the_mocks_file_from_the_argument_in_every_spelling() {
        assert_eq!(parsed(&["--mocks", "mocks.yaml"], None), serve_with_mocks("mocks.yaml"));
        assert_eq!(parsed(&["--mocks=mocks.yaml"], None), serve_with_mocks("mocks.yaml"));
        assert_eq!(parsed(&["-m", "mocks.yaml"], None), serve_with_mocks("mocks.yaml"));
    }

    #[test]
    fn reads_the_mocks_file_from_the_variable_and_the_argument_wins() {
        assert_eq!(parsed_with(&[], None, Some("from-variable.json")), serve_with_mocks("from-variable.json"));
        assert_eq!(parsed_with(&[], None, Some("")), serve(DEFAULT_PORT));
        assert_eq!(
            parsed_with(&["--mocks", "from-argument.json"], None, Some("from-variable.json")),
            serve_with_mocks("from-argument.json")
        );
    }

    #[test]
    fn reads_the_port_and_the_mocks_file_together() {
        assert_eq!(
            parsed(&["--port", "8080", "--mocks", "mocks.yaml"], None),
            Ok(Command::Serve { port: 8080, mocks: Some(PathBuf::from("mocks.yaml")) })
        );
    }

    #[test]
    fn refuses_a_mocks_argument_without_a_value() {
        assert_eq!(parsed(&["--mocks"], None), Err(ConfigError::MissingMocksValue));
    }

    #[test]
    fn serves_on_the_default_port_when_nothing_is_given() {
        assert_eq!(parsed(&[], None), serve(DEFAULT_PORT));
        assert_eq!(parsed(&[], Some("")), serve(DEFAULT_PORT));
    }

    #[test]
    fn reads_the_port_from_the_argument_in_every_spelling() {
        assert_eq!(parsed(&["--port", "8080"], None), serve(8080));
        assert_eq!(parsed(&["--port=8080"], None), serve(8080));
        assert_eq!(parsed(&["-p", "8080"], None), serve(8080));
    }

    #[test]
    fn reads_the_port_from_the_variable() {
        assert_eq!(parsed(&[], Some("9000")), serve(9000));
    }

    #[test]
    fn argument_wins_over_the_variable() {
        assert_eq!(parsed(&["--port", "8080"], Some("9000")), serve(8080));
        assert_eq!(parsed(&["--port", "8080"], Some("not a port")), serve(8080));
    }

    #[test]
    fn refuses_a_value_that_is_not_a_port() {
        for value in ["abc", "0", "70000", "-1", ""] {
            assert_eq!(parsed(&["--port", value], None), Err(ConfigError::InvalidPort(value.to_string())));
        }

        assert_eq!(parsed(&[], Some("abc")), Err(ConfigError::InvalidPort("abc".to_string())));
    }

    #[test]
    fn refuses_a_port_argument_without_a_value() {
        assert_eq!(parsed(&["--port"], None), Err(ConfigError::MissingPortValue));
    }

    #[test]
    fn refuses_an_argument_it_does_not_know() {
        assert_eq!(parsed(&["--verbose"], None), Err(ConfigError::UnknownArgument("--verbose".to_string())));
    }

    #[test]
    fn help_is_answered_before_anything_else_is_checked() {
        assert_eq!(parsed(&["--help"], Some("abc")), Ok(Command::Help));
        assert_eq!(parsed(&["-h"], None), Ok(Command::Help));
    }
}
