use super::{
    cleanup_finalization_stage, cleanup_workspace, ensure_workspace, finalize_payload,
    finalized_file_matches, plan_workspace, prepare_payload_file, DownloadFailure, DownloadJob,
    DownloadJobState, DownloadManager, DownloadManagerSnapshot, DownloadPolicy, DownloadRequest,
    DownloadStore, DownloadTransportRegistry, PersistedDownloadState,
};
use crate::error::{BackendError, BackendResult};
use std::{
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const TRANSFER_BUFFER_BYTES: usize = 256 * 1024;
const PROGRESS_CHECKPOINT_BYTES: u64 = 1024 * 1024;
const PROGRESS_NOTIFICATION_INTERVAL: Duration = Duration::from_millis(200);

type DownloadChangeNotifier = Arc<dyn Fn() + Send + Sync>;

#[derive(Clone)]
pub struct DownloadExecutionRuntime {
    inner: Arc<DownloadExecutionInner>,
}

struct DownloadExecutionInner {
    manager: Mutex<DownloadManager>,
    store: DownloadStore,
    workspace_root: PathBuf,
    destination_root: PathBuf,
    transports: DownloadTransportRegistry,
    scheduler_error: Mutex<Option<DownloadFailure>>,
    change_notifier: Mutex<Option<DownloadChangeNotifier>>,
    last_progress_notice: Mutex<Option<Instant>>,
}

impl DownloadExecutionRuntime {
    pub fn new(
        policy: DownloadPolicy,
        store: DownloadStore,
        workspace_root: impl Into<PathBuf>,
        destination_root: impl Into<PathBuf>,
        transports: DownloadTransportRegistry,
    ) -> BackendResult<Self> {
        let workspace_root = workspace_root.into();
        let destination_root = destination_root.into();
        let mut persisted = store.load()?;
        reconcile_persisted_state(&mut persisted, &workspace_root, &destination_root)?;
        let manager = DownloadManager::recover(policy, persisted)?;
        store.save(&manager.persisted_state())?;

        let runtime = Self {
            inner: Arc::new(DownloadExecutionInner {
                manager: Mutex::new(manager),
                store,
                workspace_root,
                destination_root,
                transports,
                scheduler_error: Mutex::new(None),
                change_notifier: Mutex::new(None),
                last_progress_notice: Mutex::new(None),
            }),
        };
        runtime.pump()?;
        Ok(runtime)
    }

    pub(crate) fn set_change_notifier(&self, notifier: DownloadChangeNotifier) {
        let mut slot = self
            .inner
            .change_notifier
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *slot = Some(notifier);
    }

    pub fn snapshot(&self) -> BackendResult<DownloadManagerSnapshot> {
        let manager = self.lock_manager()?;
        let mut snapshot = manager.snapshot();
        snapshot.scheduler_error = self
            .inner
            .scheduler_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        Ok(snapshot)
    }

    #[cfg(test)]
    pub fn queue(&self, request: DownloadRequest) -> BackendResult<DownloadJob> {
        self.queue_to(request, None)
    }

    pub fn queue_to(
        &self,
        request: DownloadRequest,
        destination_directory: Option<PathBuf>,
    ) -> BackendResult<DownloadJob> {
        let job =
            self.mutate_persist(|manager| manager.enqueue_to(request, destination_directory))?;
        self.pump_best_effort();
        Ok(job)
    }

    pub fn cancel(&self, job_id: &str) -> BackendResult<DownloadJob> {
        self.mutate_persist(|manager| manager.request_cancel(job_id))
    }

    pub fn retry(&self, job_id: &str) -> BackendResult<DownloadJob> {
        let job = self.mutate_persist(|manager| manager.retry(job_id))?;
        self.pump_best_effort();
        Ok(job)
    }

    pub fn remove_terminal(&self, job_id: &str) -> BackendResult<DownloadManagerSnapshot> {
        self.mutate_persist(|manager| {
            manager.remove_terminal(job_id)?;
            Ok(manager.snapshot())
        })
        .map(|mut snapshot| {
            snapshot.scheduler_error = self
                .inner
                .scheduler_error
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone();
            snapshot
        })
    }

    pub fn pump(&self) -> BackendResult<usize> {
        let claimed = {
            let mut manager = self.lock_manager()?;
            let mut candidate = manager.clone();
            let claimed = candidate.claim_ready_jobs();
            if claimed.is_empty() {
                self.clear_scheduler_error();
                return Ok(0);
            }
            self.inner.store.save(&candidate.persisted_state())?;
            *manager = candidate;
            claimed
        };
        let count = claimed.len();
        self.clear_scheduler_error();
        self.notify_change();

        for job in claimed {
            let runtime = self.clone();
            thread::spawn(move || {
                if let Err(error) = runtime.execute_claimed_job(&job.id) {
                    runtime.fail_from_backend_error(&job.id, error);
                }
                runtime.pump_best_effort();
            });
        }
        Ok(count)
    }

    fn pump_best_effort(&self) {
        if self.pump().is_err() {
            self.record_scheduler_error();
        }
    }

    fn record_scheduler_error(&self) {
        let mut slot = self
            .inner
            .scheduler_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *slot = Some(DownloadFailure {
            code: "download_scheduler_failed".into(),
            message: "Download scheduler could not continue automatically.".into(),
            retryable: true,
        });
        drop(slot);
        self.notify_change();
    }

    fn clear_scheduler_error(&self) {
        let mut slot = self
            .inner
            .scheduler_error
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *slot = None;
    }

    fn execute_claimed_job(&self, job_id: &str) -> BackendResult<()> {
        let job = self.current_job(job_id)?;
        let destination_root = job
            .destination_directory
            .as_deref()
            .unwrap_or(&self.inner.destination_root);
        let plan = plan_workspace(
            &self.inner.workspace_root,
            destination_root,
            &job.id,
            &job.destination_file_name,
        )?;

        if self.acknowledge_cancel_if_requested(job_id, &plan)? {
            return Ok(());
        }

        let mut stream = match self.inner.transports.open(&job.source) {
            Ok(stream) => stream,
            Err(failure) => {
                self.fail_job(
                    job_id,
                    &plan,
                    &failure.code,
                    &failure.message,
                    failure.retryable,
                )?;
                return Ok(());
            }
        };

        if self.acknowledge_cancel_if_requested(job_id, &plan)? {
            return Ok(());
        }

        ensure_workspace(&plan)?;
        let mut payload = prepare_payload_file(&plan)?;

        let transition = self.mutate_persist(|manager| {
            manager.mark_transferring(job_id)?;
            manager.report_progress(job_id, 0, stream.total_bytes)
        });
        if let Err(error) = transition {
            if self.acknowledge_cancel_if_requested(job_id, &plan)? {
                return Ok(());
            }
            self.fail_job(job_id, &plan, error.code(), error.message(), false)?;
            return Ok(());
        }

        let mut buffer = vec![0_u8; TRANSFER_BUFFER_BYTES];
        let mut downloaded = 0_u64;

        loop {
            if self.acknowledge_cancel_if_requested(job_id, &plan)? {
                return Ok(());
            }

            let read = match stream.reader.read(&mut buffer) {
                Ok(read) => read,
                Err(error) => {
                    let (code, retryable) = transfer_read_failure(error.kind());
                    self.fail_job(
                        job_id,
                        &plan,
                        code,
                        &format!("Download transport read failed: {error}"),
                        retryable,
                    )?;
                    return Ok(());
                }
            };
            if read == 0 {
                break;
            }

            if let Err(error) = payload.write_all(&buffer[..read]) {
                self.fail_job(
                    job_id,
                    &plan,
                    "download_payload_write_failed",
                    &format!("SearchNow could not write the download payload: {error}"),
                    true,
                )?;
                return Ok(());
            }

            downloaded = downloaded.saturating_add(read as u64);
            if let Err(error) = self.report_progress(job_id, downloaded, stream.total_bytes) {
                self.fail_job(job_id, &plan, error.code(), error.message(), false)?;
                return Ok(());
            }
        }

        if let Err(error) = payload.sync_all() {
            self.fail_job(
                job_id,
                &plan,
                "download_payload_sync_failed",
                &format!("SearchNow could not sync the completed download payload: {error}"),
                true,
            )?;
            return Ok(());
        }
        drop(payload);

        if stream
            .total_bytes
            .is_some_and(|total_bytes| downloaded != total_bytes)
        {
            self.fail_job(
                job_id,
                &plan,
                "download_transfer_incomplete",
                "Download transport ended before its declared byte count was received.",
                true,
            )?;
            return Ok(());
        }

        self.mutate_persist(|manager| manager.begin_finalizing(job_id))?;

        let final_path = match finalize_payload(&plan) {
            Ok(path) => path,
            Err(error) => {
                self.fail_job(job_id, &plan, error.code(), error.message(), true)?;
                return Ok(());
            }
        };

        if let Some(file_name) = final_path.file_name().and_then(|value| value.to_str()) {
            if file_name != job.destination_file_name {
                self.mutate_persist(|manager| {
                    manager.set_destination_file_name(job_id, file_name.to_string())
                })?;
            }
        }
        self.mutate_persist(|manager| manager.mark_completed(job_id))?;
        let _cleanup_result = cleanup_workspace(&plan);
        Ok(())
    }

    fn report_progress(
        &self,
        job_id: &str,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    ) -> BackendResult<DownloadJob> {
        let mut manager = self.lock_manager()?;
        let previous = manager.downloaded_bytes(job_id)?;
        let crossed_checkpoint =
            previous / PROGRESS_CHECKPOINT_BYTES != downloaded_bytes / PROGRESS_CHECKPOINT_BYTES;
        let completed_declared_size = total_bytes.is_some_and(|total| downloaded_bytes == total);

        if crossed_checkpoint || completed_declared_size {
            let mut candidate = manager.clone();
            let output = candidate.report_progress(job_id, downloaded_bytes, total_bytes)?;
            self.inner.store.save(&candidate.persisted_state())?;
            *manager = candidate;
            drop(manager);
            self.notify_progress_change();
            return Ok(output);
        }

        let output = manager.report_progress(job_id, downloaded_bytes, total_bytes)?;
        drop(manager);
        self.notify_progress_change();
        Ok(output)
    }

    fn acknowledge_cancel_if_requested(
        &self,
        job_id: &str,
        plan: &super::DownloadWorkspacePlan,
    ) -> BackendResult<bool> {
        if self.current_job(job_id)?.state != DownloadJobState::CancelRequested {
            return Ok(false);
        }

        self.mutate_persist(|manager| manager.acknowledge_cancel(job_id))?;
        let _cleanup_result = cleanup_workspace(plan);
        Ok(true)
    }

    fn fail_job(
        &self,
        job_id: &str,
        plan: &super::DownloadWorkspacePlan,
        code: &str,
        message: &str,
        retryable: bool,
    ) -> BackendResult<()> {
        if self.acknowledge_cancel_if_requested(job_id, plan)? {
            return Ok(());
        }

        self.mutate_persist(|manager| {
            manager.mark_failed(job_id, code.to_string(), message.to_string(), retryable)
        })?;
        let _cleanup_result = cleanup_workspace(plan);
        Ok(())
    }

    fn fail_from_backend_error(&self, job_id: &str, error: BackendError) {
        let Ok(job) = self.current_job(job_id) else {
            return;
        };
        if !job.state.is_active() {
            return;
        }

        let destination_root = job
            .destination_directory
            .as_deref()
            .unwrap_or(&self.inner.destination_root);
        let Ok(plan) = plan_workspace(
            &self.inner.workspace_root,
            destination_root,
            &job.id,
            &job.destination_file_name,
        ) else {
            self.record_scheduler_error();
            return;
        };
        if self
            .fail_job(job_id, &plan, error.code(), error.message(), true)
            .is_err()
        {
            self.record_scheduler_error();
        }
    }

    fn current_job(&self, job_id: &str) -> BackendResult<DownloadJob> {
        let manager = self.lock_manager()?;
        manager.download_job(job_id)
    }

    fn notify_change(&self) {
        let notifier = self
            .inner
            .change_notifier
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if let Some(notifier) = notifier {
            notifier();
        }
    }

    fn notify_progress_change(&self) {
        let now = Instant::now();
        let should_notify = {
            let mut last = self
                .inner
                .last_progress_notice
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if last.is_some_and(|previous| {
                now.duration_since(previous) < PROGRESS_NOTIFICATION_INTERVAL
            }) {
                false
            } else {
                *last = Some(now);
                true
            }
        };
        if should_notify {
            self.notify_change();
        }
    }

    fn mutate_persist<T>(
        &self,
        action: impl FnOnce(&mut DownloadManager) -> BackendResult<T>,
    ) -> BackendResult<T> {
        let mut manager = self.lock_manager()?;
        let mut candidate = manager.clone();
        let output = action(&mut candidate)?;
        self.inner.store.save(&candidate.persisted_state())?;
        *manager = candidate;
        drop(manager);
        self.notify_change();
        Ok(output)
    }

    fn lock_manager(&self) -> BackendResult<std::sync::MutexGuard<'_, DownloadManager>> {
        self.inner.manager.lock().map_err(|_| {
            BackendError::new(
                "download_state_lock_failed",
                "Download manager state is unavailable.",
            )
        })
    }
}

fn reconcile_persisted_state(
    state: &mut PersistedDownloadState,
    workspace_root: &Path,
    destination_root: &Path,
) -> BackendResult<()> {
    for job in &mut state.jobs {
        let selected_destination = job
            .destination_directory
            .as_deref()
            .unwrap_or(destination_root);
        let Ok(plan) = plan_workspace(
            workspace_root,
            selected_destination,
            &job.id,
            &job.destination_file_name,
        ) else {
            continue;
        };

        cleanup_finalization_stage(&plan)?;

        if job.state == DownloadJobState::Finalizing
            && finalized_file_matches(&plan, job.progress.downloaded_bytes)
            && job
                .progress
                .total_bytes
                .map_or(true, |total| total == job.progress.downloaded_bytes)
        {
            job.state = DownloadJobState::Completed;
            job.last_error = None;
            job.updated_at_ms = now_ms();
        }

        if job.state.is_terminal() {
            let _cleanup_result = cleanup_workspace(&plan);
        }
    }
    Ok(())
}

fn transfer_read_failure(kind: ErrorKind) -> (&'static str, bool) {
    match kind {
        ErrorKind::TimedOut | ErrorKind::WouldBlock => ("download_transfer_timeout", true),
        ErrorKind::InvalidData => ("download_transfer_invalid_data", false),
        _ => ("download_transfer_read_failed", true),
    }
}

fn find_job(snapshot: &DownloadManagerSnapshot, job_id: &str) -> BackendResult<DownloadJob> {
    snapshot
        .jobs
        .iter()
        .find(|job| job.id == job_id)
        .cloned()
        .ok_or_else(|| BackendError::new("download_job_not_found", "Download job was not found."))
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}

pub fn default_download_paths(app_data_root: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let root = app_data_root.join("downloads");
    (
        root.join("state.json"),
        root.join("workspace"),
        root.join("files"),
    )
}
