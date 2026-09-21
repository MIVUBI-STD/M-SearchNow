use crate::{
    diagnostics::{BackendDiagnosticsSnapshot, BackendHealthState, BackendStartupPhase},
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
        diagnostics: &BackendDiagnosticsSnapshot,
        providers: &[ProviderRuntimeStatus],
    ) -> Self {
        let lifecycle = match (diagnostics.health.startup_phase, diagnostics.health.state) {
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
