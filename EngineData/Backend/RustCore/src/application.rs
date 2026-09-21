use crate::{
    diagnostics::{BackendHealthSnapshot, BackendHealthState, BackendStartupPhase},
    provider_adapter::ProviderRuntimeStatus,
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ApplicationLifecycleState {
    Starting,
    Ready,
    Degraded,
    Unknown,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationCapabilities {
    pub library: bool,
    pub discover: bool,
    pub downloads: bool,
    pub settings: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ApplicationState {
    pub lifecycle: ApplicationLifecycleState,
    pub capabilities: ApplicationCapabilities,
}

impl ApplicationState {
    pub fn from_runtime(
        health: &BackendHealthSnapshot,
        providers: &[ProviderRuntimeStatus],
    ) -> Self {
        let lifecycle = match (health.startup_phase, health.state) {
            (BackendStartupPhase::Starting, _) => ApplicationLifecycleState::Starting,
            (BackendStartupPhase::Ready, BackendHealthState::Healthy) => {
                ApplicationLifecycleState::Ready
            }
            (BackendStartupPhase::Ready, BackendHealthState::Degraded) => {
                ApplicationLifecycleState::Degraded
            }
            _ => ApplicationLifecycleState::Unknown,
        };
        let core_available = matches!(
            lifecycle,
            ApplicationLifecycleState::Ready | ApplicationLifecycleState::Degraded
        );
        let discover = core_available
            && providers
                .iter()
                .any(|provider| provider.capabilities.catalog);

        Self {
            lifecycle,
            capabilities: ApplicationCapabilities {
                library: core_available,
                discover,
                downloads: core_available,
                settings: core_available,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{BackendHealthSnapshot, BackendHealthState, BackendStartupPhase};

    fn health(
        startup_phase: BackendStartupPhase,
        state: BackendHealthState,
    ) -> BackendHealthSnapshot {
        BackendHealthSnapshot {
            startup_phase,
            state,
            diagnostics_available: true,
            retained_events: 0,
            dropped_events: 0,
            warning_events: 0,
            error_events: 0,
            last_code: None,
        }
    }

    #[test]
    fn ready_runtime_exposes_core_capabilities_without_discover_provider() {
        let state = ApplicationState::from_runtime(
            &health(BackendStartupPhase::Ready, BackendHealthState::Healthy),
            &[],
        );

        assert_eq!(state.lifecycle, ApplicationLifecycleState::Ready);
        assert!(state.capabilities.library);
        assert!(state.capabilities.downloads);
        assert!(state.capabilities.settings);
        assert!(!state.capabilities.discover);
    }

    #[test]
    fn degraded_runtime_keeps_core_capabilities_available() {
        let state = ApplicationState::from_runtime(
            &health(BackendStartupPhase::Ready, BackendHealthState::Degraded),
            &[],
        );

        assert_eq!(state.lifecycle, ApplicationLifecycleState::Degraded);
        assert_ne!(state.lifecycle, ApplicationLifecycleState::Ready);
        assert!(state.capabilities.library);
        assert!(state.capabilities.downloads);
        assert!(state.capabilities.settings);
        assert!(!state.capabilities.discover);
    }

    #[test]
    fn starting_runtime_does_not_expose_product_capabilities() {
        let state = ApplicationState::from_runtime(
            &health(BackendStartupPhase::Starting, BackendHealthState::Unknown),
            &[],
        );

        assert_eq!(state.lifecycle, ApplicationLifecycleState::Starting);
        assert!(!state.capabilities.library);
        assert!(!state.capabilities.discover);
        assert!(!state.capabilities.downloads);
        assert!(!state.capabilities.settings);
    }
}
