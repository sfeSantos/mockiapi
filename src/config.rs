use std::fmt;

pub const DEFAULT_PORT: u16 = 3001;

/// Name of the environment variable that sets the port when `--port` is not given.
pub const PORT_VARIABLE: &str = "PORT";

pub const USAGE: &str = "\
Usage: mockiapi [--port <PORT>]

Options:
  -p, --port <PORT>  Port to listen on. Defaults to the PORT environment variable, then 3001.
  -h, --help         Print this help.";

/// What the command line asked for.
#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Serve { port: u16 },
    Help,
}

/// Why the server could not work out how to start.
#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// The value given for the port is not a number between 1 and 65535.
    InvalidPort(String),
    /// `--port` came last, with nothing after it.
    MissingPortValue,
    UnknownArgument(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigError::InvalidPort(value) => {
                write!(formatter, "'{value}' is not a port: use a number between 1 and 65535")
            }
            ConfigError::MissingPortValue => write!(formatter, "--port needs a value, for example --port 8080"),
            ConfigError::UnknownArgument(argument) => write!(formatter, "unknown argument '{argument}'"),
        }
    }
}

/// Reads the command line, without the program name, and the `PORT` variable.
///
/// The argument wins over the variable, and an empty variable counts as not set.
pub fn parse(
    arguments: impl IntoIterator<Item = String>,
    port_variable: Option<String>,
) -> Result<Command, ConfigError> {
    let mut arguments = arguments.into_iter();
    let mut requested_port = None;

    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "-h" | "--help" => return Ok(Command::Help),
            "-p" | "--port" => {
                requested_port = Some(arguments.next().ok_or(ConfigError::MissingPortValue)?);
            }
            other => match other.strip_prefix("--port=") {
                Some(value) => requested_port = Some(value.to_string()),
                None => return Err(ConfigError::UnknownArgument(argument)),
            },
        }
    }

    let port = requested_port
        .or(port_variable.filter(|value| !value.trim().is_empty()))
        .map(|value| parse_port(&value))
        .transpose()?
        .unwrap_or(DEFAULT_PORT);

    Ok(Command::Serve { port })
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
        let arguments = arguments
            .iter()
            .map(|argument| argument.to_string());

        parse(arguments, port_variable.map(str::to_string))
    }

    #[test]
    fn serves_on_the_default_port_when_nothing_is_given() {
        assert_eq!(parsed(&[], None), Ok(Command::Serve { port: DEFAULT_PORT }));
        assert_eq!(parsed(&[], Some("")), Ok(Command::Serve { port: DEFAULT_PORT }));
    }

    #[test]
    fn reads_the_port_from_the_argument_in_every_spelling() {
        assert_eq!(parsed(&["--port", "8080"], None), Ok(Command::Serve { port: 8080 }));
        assert_eq!(parsed(&["--port=8080"], None), Ok(Command::Serve { port: 8080 }));
        assert_eq!(parsed(&["-p", "8080"], None), Ok(Command::Serve { port: 8080 }));
    }

    #[test]
    fn reads_the_port_from_the_variable() {
        assert_eq!(parsed(&[], Some("9000")), Ok(Command::Serve { port: 9000 }));
    }

    #[test]
    fn argument_wins_over_the_variable() {
        assert_eq!(parsed(&["--port", "8080"], Some("9000")), Ok(Command::Serve { port: 8080 }));
        assert_eq!(parsed(&["--port", "8080"], Some("not a port")), Ok(Command::Serve { port: 8080 }));
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
