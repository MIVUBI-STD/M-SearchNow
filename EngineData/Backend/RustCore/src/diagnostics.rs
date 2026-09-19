use serde::Serialize;
use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub const DEFAULT_DIAGNOSTIC_CAPACITY: usize = 128;
pub const MAX_DIAGNOSTIC_CAPACITY: usize = 512;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticComponent {
    Runtime,
    Settings,
    Minecraft,
    Library,
    Package,
    Catalog,
    Provider,
    Download,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BackendStartupPhase {
    Starting,
    Ready,
    Unknown,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BackendHealthState {
    Healthy,
    Degraded,
    Unknown,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticEvent {
    pub timestamp_ms: u64,
    pub component: DiagnosticComponent,
    pub severity: DiagnosticSeverity,
    pub code: &'static str,
    pub message: &'static str,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackendHealthSnapshot {
    pub startup_phase: BackendStartupPhase,
    pub state: BackendHealthState,
    pub diagnostics_available: bool,
    pub retained_events: usize,
    pub dropped_events: u64,
    pub warning_events: usize,
    pub error_events: usize,
    pub last_code: Option<&'static str>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackendDiagnosticsSnapshot {
    pub health: BackendHealthSnapshot,
    pub events: Vec<DiagnosticEvent>,
}

#[derive(Clone)]
pub struct DiagnosticsBuffer {
    inner: Arc<Mutex<DiagnosticsInner>>,
    capacity: usize,
}

struct DiagnosticsInner {
    startup_phase: BackendStartupPhase,
    events: VecDeque<DiagnosticEvent>,
    dropped_events: u64,
    component_health: HashMap<DiagnosticComponent, bool>,
}

impl Default for DiagnosticsBuffer {
    fn default() -> Self {
        Self::new(DEFAULT_DIAGNOSTIC_CAPACITY)
    }
}

impl DiagnosticsBuffer {
    pub fn new(capacity: usize) -> Self {
        let capacity = capacity.clamp(1, MAX_DIAGNOSTIC_CAPACITY);
        Self {
            inner: Arc::new(Mutex::new(DiagnosticsInner {
                startup_phase: BackendStartupPhase::Starting,
                events: VecDeque::with_capacity(capacity),
                dropped_events: 0,
                component_health: HashMap::new(),
            })),
            capacity,
        }
    }

    pub fn mark_ready(&self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.startup_phase = BackendStartupPhase::Ready;
        }
    }

    pub fn record(
        &self,
        component: DiagnosticComponent,
        severity: DiagnosticSeverity,
        code: &'static str,
        message: &'static str,
        duration: Option<Duration>,
    ) {
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        push_event(
            &mut inner,
            self.capacity,
            DiagnosticEvent {
                timestamp_ms: unix_timestamp_ms(),
                component,
                severity,
                code,
                message,
                duration_ms: duration.map(duration_ms),
            },
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_outcome(
        &self,
        component: DiagnosticComponent,
        started: Instant,
        success: bool,
        success_code: &'static str,
        success_message: &'static str,
        failure_code: &'static str,
        failure_message: &'static str,
        failure_severity: DiagnosticSeverity,
    ) {
        let (severity, code, message) = if success {
            (DiagnosticSeverity::Info, success_code, success_message)
        } else {
            (failure_severity, failure_code, failure_message)
        };
        let Ok(mut inner) = self.inner.lock() else {
            return;
        };
        inner.component_health.insert(component, success);
        push_event(
            &mut inner,
            self.capacity,
            DiagnosticEvent {
                timestamp_ms: unix_timestamp_ms(),
                component,
                severity,
                code,
                message,
                duration_ms: Some(duration_ms(started.elapsed())),
            },
        );
    }

    pub fn snapshot(&self) -> BackendDiagnosticsSnapshot {
        let Ok(inner) = self.inner.lock() else {
            return BackendDiagnosticsSnapshot {
                health: BackendHealthSnapshot {
                    startup_phase: BackendStartupPhase::Unknown,
                    state: BackendHealthState::Unknown,
                    diagnostics_available: false,
                    retained_events: 0,
                    dropped_events: 0,
                    warning_events: 0,
                    error_events: 0,
                    last_code: None,
                },
                events: Vec::new(),
            };
        };

        let warning_events = inner
            .events
            .iter()
            .filter(|event| event.severity == DiagnosticSeverity::Warning)
            .count();
        let error_events = inner
            .events
            .iter()
            .filter(|event| event.severity == DiagnosticSeverity::Error)
            .count();
        let state = match inner.startup_phase {
            BackendStartupPhase::Ready => {
                if inner.component_health.values().any(|healthy| !healthy) {
                    BackendHealthState::Degraded
                } else {
                    BackendHealthState::Healthy
                }
            }
            BackendStartupPhase::Starting | BackendStartupPhase::Unknown => {
                BackendHealthState::Unknown
            }
        };
        BackendDiagnosticsSnapshot {
            health: BackendHealthSnapshot {
                startup_phase: inner.startup_phase,
                state,
                diagnostics_available: true,
                retained_events: inner.events.len(),
                dropped_events: inner.dropped_events,
                warning_events,
                error_events,
                last_code: inner.events.back().map(|event| event.code),
            },
            events: inner.events.iter().cloned().collect(),
        }
    }
}

fn push_event(inner: &mut DiagnosticsInner, capacity: usize, event: DiagnosticEvent) {
    if inner.events.len() == capacity {
        inner.events.pop_front();
        inner.dropped_events = inner.dropped_events.saturating_add(1);
    }
    inner.events.push_back(event);
}

fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

fn duration_ms(duration: Duration) -> u64 {
    duration.as_millis().min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_is_bounded_and_reports_dropped_events() {
        let diagnostics = DiagnosticsBuffer::new(2);
        for code in ["first", "second", "third"] {
            diagnostics.record(
                DiagnosticComponent::Runtime,
                DiagnosticSeverity::Info,
                code,
                "Safe static diagnostic message.",
                None,
            );
        }
        let snapshot = diagnostics.snapshot();
        assert_eq!(snapshot.events.len(), 2);
        assert_eq!(snapshot.health.dropped_events, 1);
        assert_eq!(snapshot.events[0].code, "second");
        assert_eq!(snapshot.events[1].code, "third");
    }

    #[test]
    fn ready_health_snapshot_contains_only_safe_static_event_fields() {
        let diagnostics = DiagnosticsBuffer::default();
        diagnostics.mark_ready();
        diagnostics.record(
            DiagnosticComponent::Runtime,
            DiagnosticSeverity::Info,
            "backend_runtime_ready",
            "SearchNow backend runtime is ready.",
            Some(Duration::from_millis(4)),
        );
        let snapshot = diagnostics.snapshot();
        assert_eq!(snapshot.health.startup_phase, BackendStartupPhase::Ready);
        assert_eq!(snapshot.health.state, BackendHealthState::Healthy);
        let json = serde_json::to_string(&snapshot).expect("diagnostics json");
        assert!(!json.contains("Authorization"));
        assert!(!json.contains("Bearer "));
        assert!(!json.contains("token="));
    }

    #[test]
    fn recovered_component_is_healthy_even_while_old_error_event_is_retained() {
        let diagnostics = DiagnosticsBuffer::default();
        diagnostics.mark_ready();
        diagnostics.record_outcome(
            DiagnosticComponent::Settings,
            Instant::now(),
            false,
            "settings_ok",
            "Settings loaded.",
            "settings_failed",
            "Settings failed.",
            DiagnosticSeverity::Error,
        );
        assert_eq!(
            diagnostics.snapshot().health.state,
            BackendHealthState::Degraded
        );

        diagnostics.record_outcome(
            DiagnosticComponent::Settings,
            Instant::now(),
            true,
            "settings_ok",
            "Settings loaded.",
            "settings_failed",
            "Settings failed.",
            DiagnosticSeverity::Error,
        );
        let snapshot = diagnostics.snapshot();
        assert_eq!(snapshot.health.state, BackendHealthState::Healthy);
        assert_eq!(snapshot.health.error_events, 1);
    }
}
