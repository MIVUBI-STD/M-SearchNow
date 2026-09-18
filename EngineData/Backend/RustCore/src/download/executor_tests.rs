use super::*;
use ring::digest::{digest, SHA256};
use std::{
    io::{Cursor, Read},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

#[derive(Clone)]
struct FixtureTransport {
    key: String,
    payload: Vec<u8>,
    delay: Duration,
}

impl FixtureTransport {
    fn new(key: &str, payload: Vec<u8>, delay: Duration) -> Self {
        Self {
            key: key.to_string(),
            payload,
            delay,
        }
    }
}

impl DownloadTransport for FixtureTransport {
    fn key(&self) -> &str {
        &self.key
    }

    fn open(
        &self,
        source: &DownloadSourceRef,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        self.open_from(source, 0)
    }

    fn open_from(
        &self,
        _source: &DownloadSourceRef,
        offset: u64,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        if offset > self.payload.len() as u64 {
            return Err(DownloadTransportFailure::new(
                "fixture_resume_offset_invalid",
                "Fixture resume offset exceeds payload size.",
                false,
            ));
        }
        let mut cursor = Cursor::new(self.payload.clone());
        cursor.set_position(offset);
        Ok(DownloadTransportStream {
            reader: Box::new(SlowReader {
                cursor,
                delay: self.delay,
                max_chunk: 32 * 1024,
            }),
            total_bytes: Some(self.payload.len() as u64),
        })
    }
}

struct SlowReader {
    cursor: Cursor<Vec<u8>>,
    delay: Duration,
    max_chunk: usize,
}

impl Read for SlowReader {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        if self.delay > Duration::ZERO {
            thread::sleep(self.delay);
        }
        let len = buffer.len().min(self.max_chunk);
        self.cursor.read(&mut buffer[..len])
    }
}

fn sha256_hex(payload: &[u8]) -> String {
    let digest = digest(&SHA256, payload);
    let mut output = String::with_capacity(64);
    for byte in digest.as_ref() {
        use std::fmt::Write as _;
        write!(&mut output, "{byte:02x}").expect("write digest");
    }
    output
}

fn request(transport: &str, resource_id: String, name: &str, bytes: usize) -> DownloadRequest {
    DownloadRequest {
        source: DownloadSourceRef {
            transport: transport.to_string(),
            resource_id,
        },
        display_name: name.to_string(),
        destination_file_name: format!("{name}.mcpack"),
        expected_bytes: Some(bytes as u64),
        expected_sha256: None,
    }
}

fn wait_for(
    runtime: &DownloadExecutionRuntime,
    predicate: impl Fn(&DownloadManagerSnapshot) -> bool,
) -> DownloadManagerSnapshot {
    let started = Instant::now();
    loop {
        let snapshot = runtime.snapshot().expect("snapshot");
        if predicate(&snapshot) {
            return snapshot;
        }
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "download runtime condition timed out: {snapshot:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn fixture_runtime(
    root: &std::path::Path,
    policy: DownloadPolicy,
    transport: Option<FixtureTransport>,
) -> DownloadExecutionRuntime {
    let mut registry = DownloadTransportRegistry::new();
    if let Some(transport) = transport {
        registry.register(Arc::new(transport)).expect("transport");
    }
    DownloadExecutionRuntime::new(
        policy,
        DownloadStore::new(root.join("state.json")),
        root.join("workspace"),
        root.join("files"),
        registry,
    )
    .expect("runtime")
}

#[test]
fn local_file_transport_completes_end_to_end() {
    let directory = tempfile::tempdir().expect("tempdir");
    let source = directory.path().join("source.mcpack");
    let payload = vec![7_u8; 700_000];
    std::fs::write(&source, &payload).expect("source");

    let runtime = DownloadExecutionRuntime::new(
        DownloadPolicy::default(),
        DownloadStore::new(directory.path().join("state.json")),
        directory.path().join("workspace"),
        directory.path().join("files"),
        DownloadTransportRegistry::with_local_file().expect("registry"),
    )
    .expect("runtime");

    let job = runtime
        .queue(request(
            "local-file",
            source.to_string_lossy().into_owned(),
            "local-copy",
            payload.len(),
        ))
        .expect("queue");

    let snapshot = wait_for(&runtime, |snapshot| {
        snapshot.jobs.iter().any(|candidate| {
            candidate.id == job.id && candidate.state == DownloadJobState::Completed
        })
    });
    let completed = snapshot
        .jobs
        .iter()
        .find(|candidate| candidate.id == job.id)
        .expect("completed job");
    assert_eq!(completed.progress.downloaded_bytes, payload.len() as u64);
    assert_eq!(
        std::fs::read(directory.path().join("files/local-copy.mcpack")).expect("final payload"),
        payload
    );
}

#[test]
fn scheduler_respects_active_concurrency() {
    let directory = tempfile::tempdir().expect("tempdir");
    let payload = vec![3_u8; 512 * 1024];
    let runtime = fixture_runtime(
        directory.path(),
        DownloadPolicy {
            max_active: 2,
            max_jobs: 10,
        },
        Some(FixtureTransport::new(
            "slow",
            payload.clone(),
            Duration::from_millis(15),
        )),
    );

    for name in ["one", "two", "three"] {
        runtime
            .queue(request("slow", name.to_string(), name, payload.len()))
            .expect("queue");
    }

    wait_for(&runtime, |snapshot| {
        snapshot.active_jobs == 2 && snapshot.queued_jobs == 1
    });
    let finished = wait_for(&runtime, |snapshot| {
        snapshot
            .jobs
            .iter()
            .all(|job| job.state == DownloadJobState::Completed)
    });
    assert_eq!(finished.jobs.len(), 3);
    assert!(finished.scheduler_error.is_none());
}

#[test]
fn active_cancellation_is_cooperative() {
    let directory = tempfile::tempdir().expect("tempdir");
    let payload = vec![4_u8; 1024 * 1024];
    let runtime = fixture_runtime(
        directory.path(),
        DownloadPolicy::default(),
        Some(FixtureTransport::new(
            "slow-cancel",
            payload.clone(),
            Duration::from_millis(20),
        )),
    );

    let job = runtime
        .queue(request(
            "slow-cancel",
            "fixture".into(),
            "cancel-me",
            payload.len(),
        ))
        .expect("queue");

    wait_for(&runtime, |snapshot| {
        snapshot.jobs.iter().any(|candidate| {
            candidate.id == job.id && candidate.state == DownloadJobState::Transferring
        })
    });
    runtime.cancel(&job.id).expect("cancel");

    let snapshot = wait_for(&runtime, |snapshot| {
        snapshot.jobs.iter().any(|candidate| {
            candidate.id == job.id && candidate.state == DownloadJobState::Cancelled
        })
    });
    assert_eq!(
        snapshot
            .jobs
            .iter()
            .find(|candidate| candidate.id == job.id)
            .expect("cancelled")
            .state,
        DownloadJobState::Cancelled
    );
    assert!(!directory.path().join("files/cancel-me.mcpack").exists());
}

#[test]
fn unavailable_transport_fails_without_retry_loop() {
    let directory = tempfile::tempdir().expect("tempdir");
    let runtime = fixture_runtime(directory.path(), DownloadPolicy::default(), None);
    let job = runtime
        .queue(request("missing", "resource".into(), "missing", 10))
        .expect("queue");

    let snapshot = wait_for(&runtime, |snapshot| {
        snapshot
            .jobs
            .iter()
            .any(|candidate| candidate.id == job.id && candidate.state == DownloadJobState::Failed)
    });
    let failed = snapshot
        .jobs
        .iter()
        .find(|candidate| candidate.id == job.id)
        .expect("failed");
    assert_eq!(
        failed.last_error.as_ref().expect("failure").code,
        "download_transport_unavailable"
    );
    assert!(!failed.last_error.as_ref().expect("failure").retryable);
}

#[test]
fn startup_reconciles_a_final_file_published_before_state_commit() {
    let directory = tempfile::tempdir().expect("tempdir");
    let store = DownloadStore::new(directory.path().join("state.json"));
    let workspace = directory.path().join("workspace");
    let files = directory.path().join("files");
    let payload = vec![9_u8; 10];

    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    let job = manager
        .enqueue(request(
            "fixture",
            "asset".into(),
            "recovered",
            payload.len(),
        ))
        .expect("queue");
    manager.claim_ready_jobs();
    manager.mark_transferring(&job.id).expect("transfer");
    manager
        .report_progress(&job.id, payload.len() as u64, Some(payload.len() as u64))
        .expect("progress");
    manager.begin_finalizing(&job.id).expect("finalizing");
    store.save(&manager.persisted_state()).expect("save state");

    std::fs::create_dir_all(&files).expect("files root");
    std::fs::write(files.join("recovered.mcpack"), &payload).expect("published final");

    let runtime = DownloadExecutionRuntime::new(
        DownloadPolicy::default(),
        store,
        workspace,
        files,
        DownloadTransportRegistry::new(),
    )
    .expect("runtime");
    let snapshot = runtime.snapshot().expect("snapshot");
    assert_eq!(snapshot.jobs[0].state, DownloadJobState::Completed);
    assert!(snapshot.jobs[0].last_error.is_none());
}

#[test]
fn startup_removes_stale_destination_stage_before_retry() {
    let directory = tempfile::tempdir().expect("tempdir");
    let store = DownloadStore::new(directory.path().join("state.json"));
    let workspace = directory.path().join("workspace");
    let files = directory.path().join("files");

    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    let job = manager
        .enqueue(request("fixture", "asset".into(), "staged", 10))
        .expect("queue");
    manager.claim_ready_jobs();
    store.save(&manager.persisted_state()).expect("save state");

    std::fs::create_dir_all(&files).expect("files root");
    let plan = plan_workspace(&workspace, &files, &job.id, "staged.mcpack").expect("plan");
    let stage = finalization_stage_path(&plan).expect("stage path");
    std::fs::write(&stage, b"stale").expect("stale stage");

    let runtime = DownloadExecutionRuntime::new(
        DownloadPolicy::default(),
        store,
        workspace,
        files,
        DownloadTransportRegistry::new(),
    )
    .expect("runtime");
    assert!(!stage.exists());
    assert_eq!(
        runtime.snapshot().expect("snapshot").jobs[0].state,
        DownloadJobState::Interrupted
    );
}

#[test]
fn paused_download_resumes_from_partial_payload_without_redownloading_prefix() {
    let directory = tempfile::tempdir().expect("tempdir");
    let payload = (0..(2 * 1024 * 1024))
        .map(|index| (index % 251) as u8)
        .collect::<Vec<_>>();
    let runtime = fixture_runtime(
        directory.path(),
        DownloadPolicy::default(),
        Some(FixtureTransport::new(
            "resumable",
            payload.clone(),
            Duration::from_millis(4),
        )),
    );

    let job = runtime
        .queue(request(
            "resumable",
            "fixture".into(),
            "resume-me",
            payload.len(),
        ))
        .expect("queue");

    wait_for(&runtime, |snapshot| {
        snapshot.jobs.iter().any(|candidate| {
            candidate.id == job.id
                && candidate.state == DownloadJobState::Transferring
                && candidate.progress.downloaded_bytes >= 128 * 1024
        })
    });
    runtime.pause(&job.id).expect("pause");

    let paused_snapshot = wait_for(&runtime, |snapshot| {
        snapshot
            .jobs
            .iter()
            .any(|candidate| candidate.id == job.id && candidate.state == DownloadJobState::Paused)
    });
    let paused = paused_snapshot
        .jobs
        .iter()
        .find(|candidate| candidate.id == job.id)
        .expect("paused job");
    assert!(paused.progress.downloaded_bytes > 0);
    assert!(paused.progress.downloaded_bytes < payload.len() as u64);
    let paused_bytes = paused.progress.downloaded_bytes;

    let partial = directory
        .path()
        .join("workspace")
        .join(&job.id)
        .join("payload.part");
    assert_eq!(
        std::fs::metadata(&partial).expect("partial payload").len(),
        paused_bytes
    );

    runtime.resume(&job.id).expect("resume");
    let completed_snapshot = wait_for(&runtime, |snapshot| {
        snapshot.jobs.iter().any(|candidate| {
            candidate.id == job.id && candidate.state == DownloadJobState::Completed
        })
    });
    let completed = completed_snapshot
        .jobs
        .iter()
        .find(|candidate| candidate.id == job.id)
        .expect("completed job");
    assert_eq!(completed.attempt, 2);
    assert_eq!(completed.progress.downloaded_bytes, payload.len() as u64);
    assert_eq!(
        std::fs::read(directory.path().join("files/resume-me.mcpack")).expect("final payload"),
        payload
    );
}

#[test]
fn matching_sha256_allows_completed_download_to_publish() {
    let directory = tempfile::tempdir().expect("tempdir");
    let payload = b"verified-searchnow-payload".repeat(32 * 1024);
    let runtime = fixture_runtime(
        directory.path(),
        DownloadPolicy::default(),
        Some(FixtureTransport::new(
            "integrity-match",
            payload.clone(),
            Duration::ZERO,
        )),
    );
    let mut request = request(
        "integrity-match",
        "fixture".into(),
        "verified",
        payload.len(),
    );
    request.expected_sha256 = Some(sha256_hex(&payload));

    let job = runtime.queue(request).expect("queue");
    let snapshot = wait_for(&runtime, |snapshot| {
        snapshot.jobs.iter().any(|candidate| {
            candidate.id == job.id && candidate.state == DownloadJobState::Completed
        })
    });
    let completed = snapshot
        .jobs
        .iter()
        .find(|candidate| candidate.id == job.id)
        .expect("completed job");
    assert!(completed.last_error.is_none());
    assert_eq!(
        std::fs::read(directory.path().join("files/verified.mcpack")).expect("final payload"),
        payload
    );
}

#[test]
fn mismatched_sha256_fails_before_final_file_is_published() {
    let directory = tempfile::tempdir().expect("tempdir");
    let payload = b"corruption-must-not-publish".repeat(32 * 1024);
    let runtime = fixture_runtime(
        directory.path(),
        DownloadPolicy::default(),
        Some(FixtureTransport::new(
            "integrity-mismatch",
            payload.clone(),
            Duration::ZERO,
        )),
    );
    let mut request = request(
        "integrity-mismatch",
        "fixture".into(),
        "rejected",
        payload.len(),
    );
    request.expected_sha256 = Some("00".repeat(32));

    let job = runtime.queue(request).expect("queue");
    let snapshot = wait_for(&runtime, |snapshot| {
        snapshot
            .jobs
            .iter()
            .any(|candidate| candidate.id == job.id && candidate.state == DownloadJobState::Failed)
    });
    let failed = snapshot
        .jobs
        .iter()
        .find(|candidate| candidate.id == job.id)
        .expect("failed job");
    assert_eq!(
        failed.last_error.as_ref().expect("failure").code,
        "download_integrity_mismatch"
    );
    assert!(!directory.path().join("files/rejected.mcpack").exists());
}
