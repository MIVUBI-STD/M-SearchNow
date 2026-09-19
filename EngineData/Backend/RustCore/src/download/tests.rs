use super::*;
use std::fs;

fn request(name: &str) -> DownloadRequest {
    DownloadRequest {
        source: DownloadSourceRef {
            transport: "fixture".into(),
            resource_id: format!("resource-{name}"),
        },
        display_name: name.into(),
        destination_file_name: format!("{name}.mcpack"),
        expected_bytes: Some(10),
        expected_sha256: None,
    }
}

#[test]
fn concurrency_claims_only_available_slots() {
    let mut manager = DownloadManager::new(DownloadPolicy {
        max_active: 2,
        max_jobs: 10,
    })
    .expect("manager");
    manager.enqueue(request("one")).expect("queue");
    manager.enqueue(request("two")).expect("queue");
    manager.enqueue(request("three")).expect("queue");
    let claimed = manager.claim_ready_jobs();
    assert_eq!(claimed.len(), 2);
    assert_eq!(manager.snapshot().active_jobs, 2);
    assert_eq!(manager.snapshot().queued_jobs, 1);
}

#[test]
fn progress_is_monotonic_and_bounded() {
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    let job = manager.enqueue(request("pack")).expect("queue");
    manager.claim_ready_jobs();
    manager.mark_transferring(&job.id).expect("transfer");
    manager
        .report_progress(&job.id, 5, Some(10))
        .expect("progress");
    let error = manager
        .report_progress(&job.id, 4, Some(10))
        .expect_err("regression must fail");
    assert_eq!(error.code(), "download_progress_regressed");
    let error = manager
        .report_progress(&job.id, 11, Some(10))
        .expect_err("overflow must fail");
    assert_eq!(error.code(), "download_progress_exceeds_total");
}

#[test]
fn queued_cancel_is_immediate_and_retryable() {
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    let job = manager.enqueue(request("pack")).expect("queue");
    let cancelled = manager.request_cancel(&job.id).expect("cancel");
    assert_eq!(cancelled.state, DownloadJobState::Cancelled);
    let retried = manager.retry(&job.id).expect("retry");
    assert_eq!(retried.state, DownloadJobState::Queued);
}

#[test]
fn active_cancel_requires_transport_acknowledgement() {
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    let job = manager.enqueue(request("pack")).expect("queue");
    manager.claim_ready_jobs();
    manager.mark_transferring(&job.id).expect("transfer");
    let cancelling = manager.request_cancel(&job.id).expect("request cancel");
    assert_eq!(cancelling.state, DownloadJobState::CancelRequested);
    let cancelled = manager.acknowledge_cancel(&job.id).expect("ack cancel");
    assert_eq!(cancelled.state, DownloadJobState::Cancelled);
}

#[test]
fn finalization_requires_complete_declared_size() {
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    let job = manager.enqueue(request("pack")).expect("queue");
    manager.claim_ready_jobs();
    manager.mark_transferring(&job.id).expect("transfer");
    manager
        .report_progress(&job.id, 9, Some(10))
        .expect("progress");
    let error = manager
        .begin_finalizing(&job.id)
        .expect_err("incomplete download must not finalize");
    assert_eq!(error.code(), "download_finalize_incomplete");
    manager
        .report_progress(&job.id, 10, Some(10))
        .expect("progress");
    manager.begin_finalizing(&job.id).expect("finalizing");
    let completed = manager.mark_completed(&job.id).expect("complete");
    assert_eq!(completed.state, DownloadJobState::Completed);
}

#[test]
fn non_retryable_failure_stays_failed() {
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    let job = manager.enqueue(request("pack")).expect("queue");
    manager.claim_ready_jobs();
    manager
        .mark_failed(&job.id, "fixture", "failed", false)
        .expect("failed");
    let error = manager.retry(&job.id).expect_err("retry must fail");
    assert_eq!(error.code(), "download_failure_not_retryable");
}

#[test]
fn recovery_marks_active_jobs_interrupted() {
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    manager.enqueue(request("pack")).expect("queue");
    manager.claim_ready_jobs();
    let persisted = manager.persisted_state();
    let recovered =
        DownloadManager::recover(DownloadPolicy::default(), persisted).expect("recover");
    assert_eq!(
        recovered.snapshot().jobs[0].state,
        DownloadJobState::Interrupted
    );
}

#[test]
fn recovery_rejects_duplicate_job_ids() {
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    manager.enqueue(request("pack")).expect("queue");
    let mut persisted = manager.persisted_state();
    persisted.jobs.push(persisted.jobs[0].clone());
    let error = DownloadManager::recover(DownloadPolicy::default(), persisted)
        .expect_err("duplicate ids must fail closed");
    assert_eq!(error.code(), "download_state_duplicate_job");
}

#[test]
fn recovery_advances_sequence_past_existing_job_ids() {
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    manager.enqueue(request("old")).expect("queue");
    let mut persisted = manager.persisted_state();
    persisted.jobs[0].id = "download-000010".into();
    persisted.next_sequence = 1;
    let mut recovered =
        DownloadManager::recover(DownloadPolicy::default(), persisted).expect("recover");
    let next = recovered.enqueue(request("next")).expect("next queue");
    assert_eq!(next.id, "download-000011");
}

#[test]
fn store_round_trip_preserves_queue() {
    let directory = tempfile::tempdir().expect("tempdir");
    let store = DownloadStore::new(directory.path().join("downloads.json"));
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    manager.enqueue(request("pack")).expect("queue");
    store.save(&manager.persisted_state()).expect("save");
    let recovered =
        DownloadManager::recover(DownloadPolicy::default(), store.load().expect("load"))
            .expect("recover");
    assert_eq!(recovered.snapshot().queued_jobs, 1);
}

#[test]
fn destination_file_name_cannot_escape_root() {
    let error = plan_workspace(
        std::path::Path::new("workspace"),
        std::path::Path::new("downloads"),
        "download-000001",
        "../escape.mcpack",
    )
    .expect_err("unsafe destination must fail");
    assert_eq!(error.code(), "download_destination_name_invalid");
}

#[test]
fn windows_reserved_destination_names_are_rejected() {
    for name in [
        "CON.mcpack",
        "nul.mcpack",
        "COM1.mcpack",
        "LPT9.mcpack",
        "pack.mcpack.",
        "pack.mcpack ",
        "bad:name.mcpack",
        "bad?.mcpack",
    ] {
        let error = validate_destination_file_name(name).expect_err("Windows-unsafe name");
        assert_eq!(error.code(), "download_destination_name_invalid");
    }
    validate_destination_file_name("valid-pack.mcpack").expect("valid name");
}

#[test]
fn finalization_keeps_existing_file_and_uses_next_available_name() {
    let directory = tempfile::tempdir().expect("tempdir");
    let workspace = directory.path().join("workspace");
    let destination = directory.path().join("downloads");
    let plan =
        plan_workspace(&workspace, &destination, "download-000001", "pack.mcpack").expect("plan");
    ensure_workspace(&plan).expect("workspace");
    fs::write(&plan.payload_path, b"complete payload").expect("payload");

    let first_path = finalize_payload(&plan).expect("first finalize");
    assert_eq!(
        first_path.file_name().and_then(|value| value.to_str()),
        Some("pack.mcpack")
    );
    assert_eq!(
        fs::read(&first_path).expect("first final file"),
        b"complete payload"
    );

    let second_path = finalize_payload(&plan).expect("keep both finalize");
    assert_eq!(
        second_path.file_name().and_then(|value| value.to_str()),
        Some("pack (2).mcpack")
    );
    assert_eq!(
        fs::read(&first_path).expect("original final file"),
        b"complete payload"
    );
    assert_eq!(
        fs::read(&second_path).expect("second final file"),
        b"complete payload"
    );
}

#[test]
fn legacy_v1_download_state_without_new_optional_fields_remains_compatible() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("downloads.json");
    fs::write(
        &path,
        br#"{
            "schemaVersion":1,
            "nextSequence":2,
            "jobs":[{
                "id":"download-000001",
                "source":{"transport":"fixture","resourceId":"resource-pack"},
                "displayName":"pack",
                "destinationFileName":"pack.mcpack",
                "state":"queued",
                "progress":{"downloadedBytes":0,"totalBytes":10},
                "attempt":0,
                "lastError":null,
                "createdAtMs":1,
                "updatedAtMs":1
            }]
        }"#,
    )
    .expect("legacy state");
    let store = DownloadStore::new(path);

    let persisted = store.load().expect("legacy download state load");
    assert_eq!(persisted.jobs.len(), 1);
    assert!(persisted.jobs[0].destination_directory.is_none());
    assert!(persisted.jobs[0].expected_sha256.is_none());

    let recovered =
        DownloadManager::recover(DownloadPolicy::default(), persisted).expect("recover legacy");
    assert_eq!(recovered.snapshot().queued_jobs, 1);
}

#[test]
fn store_rejects_invalid_json() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("downloads.json");
    fs::write(&path, b"{not-json").expect("corrupt state");
    let store = DownloadStore::new(path);

    let error = store.load().expect_err("invalid JSON must fail closed");
    assert_eq!(error.code(), "download_state_invalid_json");
}

#[test]
fn store_rejects_unsupported_schema() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("downloads.json");
    fs::write(
        &path,
        br#"{"schemaVersion":999,"nextSequence":1,"jobs":[]}"#,
    )
    .expect("future state");
    let store = DownloadStore::new(path);

    let error = store
        .load()
        .expect_err("unsupported schema must fail closed");
    assert_eq!(error.code(), "download_state_schema_unsupported");
}

#[test]
fn invalid_terminal_and_active_transitions_fail_closed() {
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    let job = manager.enqueue(request("pack")).expect("queue");

    let error = manager
        .mark_completed(&job.id)
        .expect_err("queued job must not complete");
    assert_eq!(error.code(), "download_complete_state_invalid");

    let error = manager
        .remove_terminal(&job.id)
        .expect_err("queued job must not be removed");
    assert_eq!(error.code(), "download_remove_state_invalid");

    manager.claim_ready_jobs();
    manager.mark_transferring(&job.id).expect("transfer");
    manager
        .report_progress(&job.id, 10, Some(10))
        .expect("progress");
    manager.begin_finalizing(&job.id).expect("finalize");

    let error = manager
        .request_cancel(&job.id)
        .expect_err("finalizing job must not be cancelled");
    assert_eq!(error.code(), "download_cancel_too_late");
}

#[test]
fn remove_completed_preserves_other_job_states() {
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    let completed = manager.enqueue(request("completed")).expect("completed queue");
    manager.claim_ready_jobs();
    manager
        .mark_transferring(&completed.id)
        .expect("completed transfer");
    manager
        .report_progress(&completed.id, 10, Some(10))
        .expect("completed progress");
    manager
        .begin_finalizing(&completed.id)
        .expect("completed finalize");
    manager
        .mark_completed(&completed.id)
        .expect("completed terminal");

    let cancelled = manager.enqueue(request("cancelled")).expect("cancelled queue");
    manager
        .request_cancel(&cancelled.id)
        .expect("cancelled terminal");

    assert_eq!(manager.remove_completed(), 1);
    let snapshot = manager.snapshot();
    assert_eq!(snapshot.jobs.len(), 1);
    assert_eq!(snapshot.jobs[0].id, cancelled.id);
    assert_eq!(snapshot.jobs[0].state, DownloadJobState::Cancelled);
}

#[test]
fn recovery_rejects_inconsistent_terminal_jobs() {
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    manager.enqueue(request("pack")).expect("queue");
    let base = manager.persisted_state();

    let mut completed = base.clone();
    completed.jobs[0].state = DownloadJobState::Completed;
    let error = DownloadManager::recover(DownloadPolicy::default(), completed)
        .expect_err("incomplete completed job must fail closed");
    assert_eq!(error.code(), "download_state_job_invalid");

    let mut failed = base;
    failed.jobs[0].state = DownloadJobState::Failed;
    let error = DownloadManager::recover(DownloadPolicy::default(), failed)
        .expect_err("failed job without error details must fail closed");
    assert_eq!(error.code(), "download_state_job_invalid");
}

#[test]
fn store_rejects_unknown_nested_persisted_fields() {
    let directory = tempfile::tempdir().expect("tempdir");
    let path = directory.path().join("downloads.json");
    fs::write(
        &path,
        br#"{
            "schemaVersion":1,
            "nextSequence":2,
            "jobs":[{
                "id":"download-000001",
                "source":{"transport":"fixture","resourceId":"resource-pack","unexpectedSource":true},
                "displayName":"pack",
                "destinationFileName":"pack.mcpack",
                "destinationDirectory":null,
                "state":"queued",
                "progress":{"downloadedBytes":0,"totalBytes":10},
                "attempt":0,
                "lastError":null,
                "createdAtMs":1,
                "updatedAtMs":1
            }]
        }"#,
    )
    .expect("persisted state");
    let store = DownloadStore::new(path);

    let error = store
        .load()
        .expect_err("unknown persisted fields must fail closed");
    assert_eq!(error.code(), "download_state_invalid_json");
}

#[test]
fn pause_and_resume_preserve_partial_progress() {
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    let job = manager.enqueue(request("pack")).expect("queue");
    manager.claim_ready_jobs();
    manager.mark_transferring(&job.id).expect("transfer");
    manager
        .report_progress(&job.id, 4, Some(10))
        .expect("progress");

    let pausing = manager.request_pause(&job.id).expect("pause request");
    assert_eq!(pausing.state, DownloadJobState::PauseRequested);
    manager
        .report_progress(&job.id, 6, Some(10))
        .expect("final in-flight progress");
    let paused = manager.acknowledge_pause(&job.id).expect("pause ack");
    assert_eq!(paused.state, DownloadJobState::Paused);
    assert_eq!(paused.progress.downloaded_bytes, 6);

    let resumed = manager.resume(&job.id).expect("resume");
    assert_eq!(resumed.state, DownloadJobState::Queued);
    assert_eq!(resumed.progress.downloaded_bytes, 6);
    assert!(resumed.last_error.is_none());
}

#[test]
fn malformed_sha256_is_rejected_before_enqueue() {
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    let mut malformed = request("bad-hash");
    malformed.expected_sha256 = Some("not-a-sha256".into());

    let error = manager
        .enqueue(malformed)
        .expect_err("malformed digest must fail");
    assert_eq!(error.code(), "download_integrity_digest_invalid");
}

#[test]
fn sha256_is_normalized_when_enqueued() {
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    let mut request = request("upper-hash");
    request.expected_sha256 = Some("AB".repeat(32));

    let job = manager.enqueue(request).expect("queue");
    let expected = "ab".repeat(32);
    assert_eq!(job.expected_sha256.as_deref(), Some(expected.as_str()));
}

#[test]
fn queued_jobs_can_be_reordered_without_mutating_nonqueued_jobs() {
    let mut manager = DownloadManager::new(DownloadPolicy {
        max_active: 1,
        max_jobs: 10,
    })
    .expect("manager");
    let first = manager.enqueue(request("first")).expect("first");
    let second = manager.enqueue(request("second")).expect("second");
    let third = manager.enqueue(request("third")).expect("third");

    manager
        .move_queued(&third.id, DownloadQueueMove::Earlier)
        .expect("move third earlier");
    manager
        .move_queued(&third.id, DownloadQueueMove::Earlier)
        .expect("move third first");

    let claimed = manager.claim_ready_jobs();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].id, third.id);

    manager
        .move_queued(&second.id, DownloadQueueMove::Earlier)
        .expect("move second earlier");
    let snapshot = manager.snapshot();
    assert_eq!(snapshot.jobs[0].id, third.id);
    assert_eq!(snapshot.jobs[1].id, second.id);
    assert_eq!(snapshot.jobs[2].id, first.id);
    assert_eq!(snapshot.jobs[0].state, DownloadJobState::Preparing);
}
