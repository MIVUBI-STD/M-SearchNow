use super::{
    model::{
        DownloadFailure, DownloadJob, DownloadJobState, DownloadManagerSnapshot, DownloadPolicy,
        DownloadProgress, DownloadQueueMove, DownloadRequest, PersistedDownloadState,
        DOWNLOAD_SCHEMA_VERSION,
    },
    workspace::{validate_destination_file_name, validate_job_id},
};
use crate::{
    error::{BackendError, BackendResult},
    identity::MAX_DOWNLOAD_RESOURCE_ID_BYTES,
};
use std::{
    collections::HashSet,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_TRANSPORT_KEY_BYTES: usize = 64;
const MAX_DISPLAY_NAME_BYTES: usize = 256;
const MAX_FAILURE_CODE_BYTES: usize = 64;
const MAX_FAILURE_MESSAGE_BYTES: usize = 512;
const MAX_ACTIVE_JOBS: usize = 16;
const MAX_RETAINED_JOBS: usize = 5_000;

#[derive(Debug, Clone)]
pub struct DownloadManager {
    policy: DownloadPolicy,
    next_sequence: u64,
    jobs: Vec<DownloadJob>,
}

impl DownloadManager {
    #[cfg(test)]
    pub fn new(policy: DownloadPolicy) -> BackendResult<Self> {
        validate_policy(policy)?;
        Ok(Self {
            policy,
            next_sequence: 1,
            jobs: Vec::new(),
        })
    }

    pub fn recover(
        policy: DownloadPolicy,
        persisted: PersistedDownloadState,
    ) -> BackendResult<Self> {
        validate_policy(policy)?;
        if persisted.schema_version != DOWNLOAD_SCHEMA_VERSION {
            return Err(BackendError::new(
                "download_state_schema_unsupported",
                "Persisted download state schema is not supported.",
            ));
        }
        if persisted.jobs.len() > policy.max_jobs {
            return Err(BackendError::new(
                "download_state_job_limit",
                "Persisted download state exceeds the configured job limit.",
            ));
        }

        let mut seen_ids = HashSet::with_capacity(persisted.jobs.len());
        let mut max_sequence = 0_u64;
        for job in &persisted.jobs {
            validate_persisted_job(job)?;
            if !seen_ids.insert(job.id.as_str()) {
                return Err(BackendError::new(
                    "download_state_duplicate_job",
                    "Persisted download state contains duplicate job identities.",
                ));
            }
            max_sequence = max_sequence.max(parse_job_sequence(&job.id)?);
        }

        if persisted.next_sequence == u64::MAX || max_sequence == u64::MAX {
            return Err(BackendError::new(
                "download_state_sequence_invalid",
                "Persisted download sequence cannot be safely advanced.",
            ));
        }

        let now = now_ms();
        let mut jobs = persisted.jobs;
        for job in &mut jobs {
            if job.state.is_active() {
                job.state = DownloadJobState::Interrupted;
                job.last_error = Some(DownloadFailure {
                    code: "download_interrupted".into(),
                    message: "SearchNow closed before this download finished.".into(),
                    retryable: true,
                });
                job.updated_at_ms = now;
            }
        }

        Ok(Self {
            policy,
            next_sequence: persisted
                .next_sequence
                .max(max_sequence.saturating_add(1))
                .max(1),
            jobs,
        })
    }

    pub fn snapshot(&self) -> DownloadManagerSnapshot {
        DownloadManagerSnapshot {
            policy: self.policy,
            active_jobs: self.jobs.iter().filter(|job| job.state.is_active()).count(),
            queued_jobs: self
                .jobs
                .iter()
                .filter(|job| job.state == DownloadJobState::Queued)
                .count(),
            jobs: self.jobs.clone(),
            scheduler_error: None,
        }
    }

    pub fn persisted_state(&self) -> PersistedDownloadState {
        PersistedDownloadState {
            schema_version: DOWNLOAD_SCHEMA_VERSION,
            next_sequence: self.next_sequence,
            jobs: self.jobs.clone(),
        }
    }

    pub(crate) fn downloaded_bytes(&self, job_id: &str) -> BackendResult<u64> {
        self.jobs
            .iter()
            .find(|job| job.id == job_id)
            .map(|job| job.progress.downloaded_bytes)
            .ok_or_else(|| {
                BackendError::new("download_job_not_found", "Download job was not found.")
            })
    }

    pub(crate) fn download_job(&self, job_id: &str) -> BackendResult<DownloadJob> {
        self.jobs
            .iter()
            .find(|job| job.id == job_id)
            .cloned()
            .ok_or_else(|| {
                BackendError::new("download_job_not_found", "Download job was not found.")
            })
    }

    #[cfg(test)]
    pub fn enqueue(&mut self, request: DownloadRequest) -> BackendResult<DownloadJob> {
        self.enqueue_to(request, None)
    }

    pub fn enqueue_to(
        &mut self,
        request: DownloadRequest,
        destination_directory: Option<PathBuf>,
    ) -> BackendResult<DownloadJob> {
        validate_request(&request)?;
        validate_destination_directory(destination_directory.as_ref())?;
        if self.jobs.len() >= self.policy.max_jobs {
            return Err(BackendError::new(
                "download_queue_full",
                "Download history reached its configured retained-job limit.",
            ));
        }
        let id = self.next_id()?;
        let now = now_ms();
        let job = DownloadJob {
            id,
            source: request.source,
            display_name: request.display_name.trim().to_string(),
            destination_file_name: request.destination_file_name,
            destination_directory,
            state: DownloadJobState::Queued,
            progress: DownloadProgress {
                downloaded_bytes: 0,
                total_bytes: request.expected_bytes,
            },
            expected_sha256: request
                .expected_sha256
                .map(|value| value.to_ascii_lowercase()),
            attempt: 0,
            last_error: None,
            created_at_ms: now,
            updated_at_ms: now,
        };
        self.jobs.push(job.clone());
        Ok(job)
    }

    pub fn claim_ready_jobs(&mut self) -> Vec<DownloadJob> {
        let active = self.jobs.iter().filter(|job| job.state.is_active()).count();
        let mut slots = self.policy.max_active.saturating_sub(active);
        let now = now_ms();
        let mut claimed = Vec::new();
        for job in &mut self.jobs {
            if slots == 0 {
                break;
            }
            if job.state != DownloadJobState::Queued {
                continue;
            }
            job.state = DownloadJobState::Preparing;
            job.attempt = job.attempt.saturating_add(1);
            job.updated_at_ms = now;
            claimed.push(job.clone());
            slots -= 1;
        }
        claimed
    }

    pub fn mark_transferring(&mut self, job_id: &str) -> BackendResult<DownloadJob> {
        let job = self.job_mut(job_id)?;
        require_state(
            job,
            &[DownloadJobState::Preparing],
            "download_transfer_start_invalid",
        )?;
        job.state = DownloadJobState::Transferring;
        job.updated_at_ms = now_ms();
        Ok(job.clone())
    }

    pub fn report_progress(
        &mut self,
        job_id: &str,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    ) -> BackendResult<DownloadJob> {
        let job = self.job_mut(job_id)?;
        require_state(
            job,
            &[
                DownloadJobState::Transferring,
                DownloadJobState::PauseRequested,
            ],
            "download_progress_state_invalid",
        )?;
        if downloaded_bytes < job.progress.downloaded_bytes {
            return Err(BackendError::new(
                "download_progress_regressed",
                "Download progress cannot move backwards.",
            ));
        }
        let total = match (job.progress.total_bytes, total_bytes) {
            (Some(existing), Some(incoming)) if existing != incoming => {
                return Err(BackendError::new(
                    "download_total_changed",
                    "Download total size changed during the same attempt.",
                ));
            }
            (Some(existing), _) => Some(existing),
            (None, incoming) => incoming,
        };
        if total.is_some_and(|value| downloaded_bytes > value) {
            return Err(BackendError::new(
                "download_progress_exceeds_total",
                "Downloaded bytes exceed the declared total size.",
            ));
        }
        job.progress.downloaded_bytes = downloaded_bytes;
        job.progress.total_bytes = total;
        job.updated_at_ms = now_ms();
        Ok(job.clone())
    }

    pub fn begin_finalizing(&mut self, job_id: &str) -> BackendResult<DownloadJob> {
        let job = self.job_mut(job_id)?;
        require_state(
            job,
            &[DownloadJobState::Transferring],
            "download_finalize_state_invalid",
        )?;
        if job
            .progress
            .total_bytes
            .is_some_and(|total| job.progress.downloaded_bytes != total)
        {
            return Err(BackendError::new(
                "download_finalize_incomplete",
                "Download cannot finalize before all declared bytes are received.",
            ));
        }
        job.state = DownloadJobState::Finalizing;
        job.updated_at_ms = now_ms();
        Ok(job.clone())
    }

    pub fn set_destination_file_name(
        &mut self,
        job_id: &str,
        destination_file_name: String,
    ) -> BackendResult<DownloadJob> {
        validate_destination_file_name(&destination_file_name)?;
        let job = self.job_mut(job_id)?;
        require_state(
            job,
            &[DownloadJobState::Finalizing],
            "download_destination_update_invalid",
        )?;
        job.destination_file_name = destination_file_name;
        job.updated_at_ms = now_ms();
        Ok(job.clone())
    }

    pub fn mark_completed(&mut self, job_id: &str) -> BackendResult<DownloadJob> {
        let job = self.job_mut(job_id)?;
        require_state(
            job,
            &[DownloadJobState::Finalizing],
            "download_complete_state_invalid",
        )?;
        job.state = DownloadJobState::Completed;
        job.last_error = None;
        job.updated_at_ms = now_ms();
        Ok(job.clone())
    }

    pub fn mark_failed(
        &mut self,
        job_id: &str,
        code: impl Into<String>,
        message: impl Into<String>,
        retryable: bool,
    ) -> BackendResult<DownloadJob> {
        let job = self.job_mut(job_id)?;
        require_state(
            job,
            &[
                DownloadJobState::Preparing,
                DownloadJobState::Transferring,
                DownloadJobState::Finalizing,
                DownloadJobState::CancelRequested,
            ],
            "download_fail_state_invalid",
        )?;
        job.state = DownloadJobState::Failed;
        job.last_error = Some(DownloadFailure {
            code: code.into(),
            message: message.into(),
            retryable,
        });
        job.updated_at_ms = now_ms();
        Ok(job.clone())
    }

    pub fn request_pause(&mut self, job_id: &str) -> BackendResult<DownloadJob> {
        let job = self.job_mut(job_id)?;
        match job.state {
            DownloadJobState::Queued => job.state = DownloadJobState::Paused,
            DownloadJobState::Preparing | DownloadJobState::Transferring => {
                job.state = DownloadJobState::PauseRequested;
            }
            DownloadJobState::PauseRequested | DownloadJobState::Paused => {}
            DownloadJobState::Finalizing => {
                return Err(BackendError::new(
                    "download_pause_too_late",
                    "Download is already finalizing and can no longer be paused safely.",
                ));
            }
            _ => {
                return Err(BackendError::new(
                    "download_pause_state_invalid",
                    "Download cannot be paused from its current state.",
                ));
            }
        }
        job.updated_at_ms = now_ms();
        Ok(job.clone())
    }

    pub fn acknowledge_pause(&mut self, job_id: &str) -> BackendResult<DownloadJob> {
        let job = self.job_mut(job_id)?;
        require_state(
            job,
            &[DownloadJobState::PauseRequested],
            "download_pause_ack_invalid",
        )?;
        job.state = DownloadJobState::Paused;
        job.updated_at_ms = now_ms();
        Ok(job.clone())
    }

    pub fn resume(&mut self, job_id: &str) -> BackendResult<DownloadJob> {
        let job = self.job_mut(job_id)?;
        require_state(
            job,
            &[DownloadJobState::Paused, DownloadJobState::Interrupted],
            "download_resume_state_invalid",
        )?;
        job.state = DownloadJobState::Queued;
        job.last_error = None;
        job.updated_at_ms = now_ms();
        Ok(job.clone())
    }

    pub fn request_cancel(&mut self, job_id: &str) -> BackendResult<DownloadJob> {
        let job = self.job_mut(job_id)?;
        match job.state {
            DownloadJobState::Queued | DownloadJobState::Paused => {
                job.state = DownloadJobState::Cancelled;
            }
            DownloadJobState::Preparing
            | DownloadJobState::Transferring
            | DownloadJobState::PauseRequested => {
                job.state = DownloadJobState::CancelRequested;
            }
            DownloadJobState::CancelRequested => {}
            DownloadJobState::Finalizing => {
                return Err(BackendError::new(
                    "download_cancel_too_late",
                    "Download is already finalizing and can no longer be cancelled safely.",
                ));
            }
            _ => {
                return Err(BackendError::new(
                    "download_cancel_state_invalid",
                    "Download cannot be cancelled from its current state.",
                ));
            }
        }
        job.updated_at_ms = now_ms();
        Ok(job.clone())
    }

    pub fn acknowledge_cancel(&mut self, job_id: &str) -> BackendResult<DownloadJob> {
        let job = self.job_mut(job_id)?;
        require_state(
            job,
            &[DownloadJobState::CancelRequested],
            "download_cancel_ack_invalid",
        )?;
        job.state = DownloadJobState::Cancelled;
        job.updated_at_ms = now_ms();
        Ok(job.clone())
    }

    pub fn retry(&mut self, job_id: &str) -> BackendResult<DownloadJob> {
        let job = self.job_mut(job_id)?;
        if !job.state.is_retryable() {
            return Err(BackendError::new(
                "download_retry_state_invalid",
                "Download cannot be retried from its current state.",
            ));
        }
        if job.state == DownloadJobState::Failed
            && job
                .last_error
                .as_ref()
                .is_some_and(|error| !error.retryable)
        {
            return Err(BackendError::new(
                "download_failure_not_retryable",
                "This download failure was marked non-retryable.",
            ));
        }
        job.state = DownloadJobState::Queued;
        job.progress.downloaded_bytes = 0;
        job.last_error = None;
        job.updated_at_ms = now_ms();
        Ok(job.clone())
    }

    pub fn move_queued(
        &mut self,
        job_id: &str,
        direction: DownloadQueueMove,
    ) -> BackendResult<DownloadJob> {
        let current_index = self
            .jobs
            .iter()
            .position(|job| job.id == job_id)
            .ok_or_else(|| {
                BackendError::new("download_job_not_found", "Download job was not found.")
            })?;
        if self.jobs[current_index].state != DownloadJobState::Queued {
            return Err(BackendError::new(
                "download_queue_move_state_invalid",
                "Only queued downloads can be reordered.",
            ));
        }

        let target_index = match direction {
            DownloadQueueMove::Earlier => self.jobs[..current_index]
                .iter()
                .rposition(|job| job.state == DownloadJobState::Queued),
            DownloadQueueMove::Later => self.jobs[current_index + 1..]
                .iter()
                .position(|job| job.state == DownloadJobState::Queued)
                .map(|offset| current_index + 1 + offset),
        };

        if let Some(target_index) = target_index {
            self.jobs.swap(current_index, target_index);
        }
        self.download_job(job_id)
    }

    pub fn remove_terminal(&mut self, job_id: &str) -> BackendResult<()> {
        let index = self
            .jobs
            .iter()
            .position(|job| job.id == job_id)
            .ok_or_else(|| {
                BackendError::new("download_job_not_found", "Download job was not found.")
            })?;
        if !self.jobs[index].state.is_terminal() {
            return Err(BackendError::new(
                "download_remove_state_invalid",
                "Only terminal download jobs can be removed.",
            ));
        }
        self.jobs.remove(index);
        Ok(())
    }

    fn job_mut(&mut self, job_id: &str) -> BackendResult<&mut DownloadJob> {
        self.jobs
            .iter_mut()
            .find(|job| job.id == job_id)
            .ok_or_else(|| {
                BackendError::new("download_job_not_found", "Download job was not found.")
            })
    }

    fn next_id(&mut self) -> BackendResult<String> {
        loop {
            let sequence = self.next_sequence;
            self.next_sequence = self.next_sequence.checked_add(1).ok_or_else(|| {
                BackendError::new(
                    "download_sequence_exhausted",
                    "Download sequence cannot be safely advanced.",
                )
            })?;
            let id = format!("download-{sequence:06}");
            if !self.jobs.iter().any(|job| job.id == id) {
                return Ok(id);
            }
        }
    }
}

fn validate_policy(policy: DownloadPolicy) -> BackendResult<()> {
    if policy.max_active == 0
        || policy.max_active > MAX_ACTIVE_JOBS
        || policy.max_jobs == 0
        || policy.max_jobs > MAX_RETAINED_JOBS
        || policy.max_active > policy.max_jobs
    {
        return Err(BackendError::new(
            "download_policy_invalid",
            "Download policy concurrency or retained-job limits are invalid.",
        ));
    }
    Ok(())
}

fn validate_request(request: &DownloadRequest) -> BackendResult<()> {
    let display_name = request.display_name.trim();
    if display_name.is_empty()
        || display_name.len() > MAX_DISPLAY_NAME_BYTES
        || display_name.chars().any(char::is_control)
    {
        return Err(BackendError::new(
            "download_display_name_invalid",
            "Download display name is empty or unsupported.",
        ));
    }
    if request.source.transport.is_empty()
        || request.source.transport.len() > MAX_TRANSPORT_KEY_BYTES
        || !request
            .source
            .transport
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return Err(BackendError::new(
            "download_transport_invalid",
            "Download transport key is empty or unsupported.",
        ));
    }
    if request.source.resource_id.is_empty()
        || request.source.resource_id.len() > MAX_DOWNLOAD_RESOURCE_ID_BYTES
        || request.source.resource_id.chars().any(char::is_control)
    {
        return Err(BackendError::new(
            "download_resource_id_invalid",
            "Download resource id is empty or unsupported.",
        ));
    }
    if request.expected_sha256.as_deref().is_some_and(|digest| {
        digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) {
        return Err(BackendError::new(
            "download_integrity_digest_invalid",
            "Expected SHA-256 digest must contain exactly 64 hexadecimal characters.",
        ));
    }
    validate_destination_file_name(&request.destination_file_name)
}

fn validate_destination_directory(path: Option<&PathBuf>) -> BackendResult<()> {
    if path.is_some_and(|path| path.as_os_str().is_empty() || !path.is_absolute()) {
        return Err(BackendError::new(
            "download_destination_directory_invalid",
            "Download destination folder must be an absolute path.",
        ));
    }
    Ok(())
}

fn validate_persisted_job(job: &DownloadJob) -> BackendResult<()> {
    validate_job_id(&job.id)?;
    let _ = parse_job_sequence(&job.id)?;
    validate_request(&DownloadRequest {
        source: job.source.clone(),
        display_name: job.display_name.clone(),
        destination_file_name: job.destination_file_name.clone(),
        expected_bytes: job.progress.total_bytes,
        expected_sha256: job.expected_sha256.clone(),
    })?;
    validate_destination_directory(job.destination_directory.as_ref())?;

    if job.created_at_ms > job.updated_at_ms
        || job
            .progress
            .total_bytes
            .is_some_and(|total| job.progress.downloaded_bytes > total)
    {
        return Err(invalid_persisted_job());
    }

    if let Some(error) = &job.last_error {
        if error.code.is_empty()
            || error.code.len() > MAX_FAILURE_CODE_BYTES
            || !error
                .code
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            || error.message.is_empty()
            || error.message.len() > MAX_FAILURE_MESSAGE_BYTES
            || error.message.chars().any(char::is_control)
        {
            return Err(invalid_persisted_job());
        }
    }

    match job.state {
        DownloadJobState::Completed => {
            if job.last_error.is_some()
                || job
                    .progress
                    .total_bytes
                    .is_some_and(|total| job.progress.downloaded_bytes != total)
            {
                return Err(invalid_persisted_job());
            }
        }
        DownloadJobState::Failed => {
            if job.last_error.is_none() {
                return Err(invalid_persisted_job());
            }
        }
        DownloadJobState::Interrupted
            if !job.last_error.as_ref().is_some_and(|error| error.retryable) =>
        {
            return Err(invalid_persisted_job());
        }
        _ => {}
    }

    Ok(())
}

fn parse_job_sequence(job_id: &str) -> BackendResult<u64> {
    let value = job_id
        .strip_prefix("download-")
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .ok_or_else(invalid_persisted_job)?;
    Ok(value)
}

fn invalid_persisted_job() -> BackendError {
    BackendError::new(
        "download_state_job_invalid",
        "Persisted download state contains a malformed or inconsistent job.",
    )
}

fn require_state(
    job: &DownloadJob,
    allowed: &[DownloadJobState],
    code: &'static str,
) -> BackendResult<()> {
    if allowed.contains(&job.state) {
        Ok(())
    } else {
        Err(BackendError::new(
            code,
            "Download state transition is not valid from the current state.",
        ))
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
