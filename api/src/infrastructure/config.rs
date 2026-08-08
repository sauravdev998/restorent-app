//! Configuration, read once at boot into a typed struct.
//!
//! Every value the process needs is read and checked here, before anything
//! opens a socket. A missing database password fails the deploy, in the logs,
//! at start up, rather than failing the first request in the middle of a dinner
//! service. Nothing else in the codebase reads an environment variable.

use std::env::{self, VarError};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use thiserror::Error;

/// Which environment this process is running in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    /// An engineer's machine. Logs are pretty, and development only routes exist.
    Development,
    /// Anything real. Logs are JSON, and development only routes do not exist.
    Production,
}

impl FromStr for Environment {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "development" | "dev" | "local" => Ok(Self::Development),
            "production" | "prod" => Ok(Self::Production),
            other => Err(ConfigError::Invalid {
                key: "APP_ENV",
                reason: format!("expected `development` or `production`, got `{other}`"),
            }),
        }
    }
}

/// Everything this process needs to run, already checked.
#[derive(Debug, Clone)]
pub struct Config {
    /// What the API connects as. A least privilege role that owns nothing, so
    /// row level security applies to it.
    pub database_url: String,
    /// Ceiling on pooled connections. The listen connection is taken from
    /// outside this pool and is not counted here.
    pub database_max_connections: u32,
    /// Where the HTTP server binds.
    pub bind_address: SocketAddr,
    /// Which environment this is.
    pub environment: Environment,
}

/// Why configuration could not be loaded.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// A required variable was not set at all.
    #[error("required environment variable `{0}` is not set")]
    Missing(&'static str),

    /// A variable was set but its value does not make sense.
    #[error("environment variable `{key}` is invalid: {reason}")]
    Invalid {
        /// Which variable.
        key: &'static str,
        /// What was wrong with it.
        reason: String,
    },
}

impl Config {
    /// Reads and validates every setting.
    ///
    /// # Errors
    ///
    /// Returns the first problem found, naming the variable, so a failed boot
    /// says exactly what to fix.
    pub fn from_env() -> Result<Self, ConfigError> {
        let database_url = required("DATABASE_URL")?;
        if !database_url.starts_with("postgres://") && !database_url.starts_with("postgresql://") {
            return Err(ConfigError::Invalid {
                key: "DATABASE_URL",
                reason: "expected a postgres:// connection string".to_owned(),
            });
        }

        let database_max_connections = parse_or_default("DATABASE_MAX_CONNECTIONS", 10u32)?;
        if database_max_connections == 0 {
            return Err(ConfigError::Invalid {
                key: "DATABASE_MAX_CONNECTIONS",
                reason: "must be at least 1".to_owned(),
            });
        }

        let host: IpAddr = parse_or_default("APP_HOST", IpAddr::from([127, 0, 0, 1]))?;
        let port: u16 = parse_or_default("APP_PORT", 8080u16)?;

        let environment = match env::var("APP_ENV") {
            Ok(raw) => raw.parse()?,
            Err(VarError::NotPresent) => Environment::Development,
            Err(VarError::NotUnicode(_)) => {
                return Err(ConfigError::Invalid {
                    key: "APP_ENV",
                    reason: "value is not valid UTF-8".to_owned(),
                });
            }
        };

        Ok(Self {
            database_url,
            database_max_connections,
            bind_address: SocketAddr::new(host, port),
            environment,
        })
    }

    /// True in development, where development only routes are mounted.
    #[must_use]
    pub fn is_development(&self) -> bool {
        self.environment == Environment::Development
    }
}

fn required(key: &'static str) -> Result<String, ConfigError> {
    match env::var(key) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        Ok(_) => Err(ConfigError::Invalid {
            key,
            reason: "is set but empty".to_owned(),
        }),
        Err(VarError::NotPresent) => Err(ConfigError::Missing(key)),
        Err(VarError::NotUnicode(_)) => Err(ConfigError::Invalid {
            key,
            reason: "value is not valid UTF-8".to_owned(),
        }),
    }
}

fn parse_or_default<T>(key: &'static str, fallback: T) -> Result<T, ConfigError>
where
    T: FromStr,
    T::Err: std::fmt::Display,
{
    match env::var(key) {
        Ok(raw) => raw.trim().parse().map_err(|error| ConfigError::Invalid {
            key,
            reason: format!("{error}"),
        }),
        Err(VarError::NotPresent) => Ok(fallback),
        Err(VarError::NotUnicode(_)) => Err(ConfigError::Invalid {
            key,
            reason: "value is not valid UTF-8".to_owned(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_parses_the_forms_people_actually_type() {
        assert_eq!(
            "development".parse::<Environment>().unwrap(),
            Environment::Development
        );
        assert_eq!(
            "PROD".parse::<Environment>().unwrap(),
            Environment::Production
        );
        assert!("staging".parse::<Environment>().is_err());
    }
}
