//! The health use case.
//!
//! Worth a use case of its own rather than a line inside a handler, because the
//! rule it encodes is load bearing: a container whose listen connection has
//! died still serves HTTP perfectly well while delivering no tickets at all. A
//! check that only proved the process was alive would keep that container in
//! service indefinitely, and the kitchen would simply stop seeing orders.

use serde::Serialize;

use super::ports::HealthPort;

/// Whether one dependency is answering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComponentState {
    /// Answering normally.
    Up,
    /// Not answering.
    Down,
}

impl ComponentState {
    const fn from_bool(ok: bool) -> Self {
        if ok { Self::Up } else { Self::Down }
    }
}

/// The state of everything this instance needs in order to be useful.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct HealthReport {
    /// Whether the connection pool answers.
    pub database: ComponentState,
    /// Whether this instance's Postgres listen connection is up.
    pub listener: ComponentState,
}

impl HealthReport {
    /// True only when every dependency is up.
    ///
    /// The load balancer takes an instance out of service on anything else,
    /// which is the intended behaviour for a dead listener.
    #[must_use]
    pub fn is_serving(&self) -> bool {
        self.database == ComponentState::Up && self.listener == ComponentState::Up
    }
}

/// Reports whether this instance can serve.
pub async fn check_health<P: HealthPort>(port: &P) -> HealthReport {
    HealthReport {
        database: ComponentState::from_bool(port.database_reachable().await),
        listener: ComponentState::from_bool(port.listener_alive()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Stub {
        database: bool,
        listener: bool,
    }

    impl HealthPort for Stub {
        async fn database_reachable(&self) -> bool {
            self.database
        }

        fn listener_alive(&self) -> bool {
            self.listener
        }
    }

    #[tokio::test]
    async fn serving_only_when_both_dependencies_are_up() {
        let both_up = check_health(&Stub {
            database: true,
            listener: true,
        })
        .await;
        assert!(both_up.is_serving());

        // The case the whole endpoint exists for: the process is fine and the
        // database is fine, but nothing is being delivered to any screen.
        let listener_dead = check_health(&Stub {
            database: true,
            listener: false,
        })
        .await;
        assert!(!listener_dead.is_serving());
        assert_eq!(listener_dead.database, ComponentState::Up);
        assert_eq!(listener_dead.listener, ComponentState::Down);

        let database_dead = check_health(&Stub {
            database: false,
            listener: true,
        })
        .await;
        assert!(!database_dead.is_serving());
    }
}
