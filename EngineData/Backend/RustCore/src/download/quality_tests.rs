use super::*;
use std::fs;

fn request(name: &str) -> DownloadRequest {
    DownloadRequest {
        source: DownloadSourceRef {
            transport: "fixture".into(),
            resource_id: format!("resource-{name}"),
        },
        display_name: name.into(),
        destination_file_name: format!("{name}.mcaddon"),
        expected_bytes: Some(7),
        expected_sha256: None,
    }
}

#[test]
fn unicode_file_name_survives_keep_both_finalization() {
    let directory = tempfile::tempdir().expect("tempdir");
    let workspace = directory.path().join("workspace");
    let destination = directory.path().join("downloads");
    let plan = plan_workspace(
        &workspace,
        &destination,
        "download-000001",
        "Peta_日本.mcaddon",
    )
    .expect("plan");

    ensure_workspace(&plan).expect("workspace");
    fs::write(&plan.payload_path, b"payload").expect("payload");

    let first = finalize_payload(&plan).expect("first finalization");
    let second = finalize_payload(&plan).expect("keep both finalization");

    assert_eq!(
        first.file_name().and_then(|value| value.to_str()),
        Some("Peta_日本.mcaddon")
    );
    assert_eq!(
        second.file_name().and_then(|value| value.to_str()),
        Some("Peta_日本 (2).mcaddon")
    );
    assert_eq!(fs::read(first).expect("first payload"), b"payload");
    assert_eq!(fs::read(second).expect("second payload"), b"payload");
}

#[test]
fn custom_destination_must_be_absolute() {
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    let error = manager
        .enqueue_to(request("pack"), Some("relative/downloads".into()))
        .expect_err("relative destination must be rejected");

    assert_eq!(error.code(), "download_destination_directory_invalid");
}

#[test]
fn retry_preserves_the_user_selected_destination() {
    let directory = tempfile::tempdir().expect("tempdir");
    let destination = directory.path().join("chosen");
    let mut manager = DownloadManager::new(DownloadPolicy::default()).expect("manager");
    let job = manager
        .enqueue_to(request("pack"), Some(destination.clone()))
        .expect("queue");

    manager.claim_ready_jobs();
    manager
        .mark_failed(&job.id, "fixture_failure", "retry me", true)
        .expect("failed");
    let retried = manager.retry(&job.id).expect("retry");

    assert_eq!(retried.destination_directory, Some(destination));
    assert_eq!(retried.state, DownloadJobState::Queued);
    assert_eq!(retried.progress.downloaded_bytes, 0);
}

#[test]
fn destination_that_changes_into_a_file_is_rejected_before_publish() {
    let directory = tempfile::tempdir().expect("tempdir");
    let workspace = directory.path().join("workspace");
    let destination = directory.path().join("chosen");
    let plan =
        plan_workspace(&workspace, &destination, "download-000002", "pack.mcaddon").expect("plan");

    ensure_workspace(&plan).expect("workspace");
    fs::write(&plan.payload_path, b"payload").expect("payload");
    fs::write(&destination, b"not a directory").expect("destination file");

    let error = finalize_payload(&plan).expect_err("non-directory destination must fail closed");
    assert_eq!(error.code(), "download_destination_directory_invalid");
    assert!(!directory.path().join("chosen/pack.mcaddon").exists());
}

#[test]
fn deleted_destination_directory_is_recreated_safely() {
    let directory = tempfile::tempdir().expect("tempdir");
    let workspace = directory.path().join("workspace");
    let destination = directory.path().join("chosen");
    fs::create_dir_all(&destination).expect("initial destination");

    let plan =
        plan_workspace(&workspace, &destination, "download-000003", "pack.mcaddon").expect("plan");
    ensure_workspace(&plan).expect("workspace");
    fs::write(&plan.payload_path, b"payload").expect("payload");
    fs::remove_dir(&destination).expect("remove destination");

    let published = finalize_payload(&plan).expect("recreate destination");
    assert_eq!(published, destination.join("pack.mcaddon"));
    assert_eq!(fs::read(published).expect("published payload"), b"payload");
}

#[test]
fn keep_both_rejects_name_that_would_exceed_windows_safe_limit() {
    let directory = tempfile::tempdir().expect("tempdir");
    let workspace = directory.path().join("workspace");
    let destination = directory.path().join("downloads");
    let long_stem = "a".repeat(231);
    let file_name = format!("{long_stem}.mcaddon");
    assert_eq!(file_name.len(), 239);

    let plan = plan_workspace(&workspace, &destination, "download-000004", &file_name)
        .expect("base file name should fit");
    ensure_workspace(&plan).expect("workspace");
    fs::write(&plan.payload_path, b"payload").expect("payload");

    finalize_payload(&plan).expect("first publish");
    let error = finalize_payload(&plan).expect_err("keep-both suffix should exceed limit");
    assert_eq!(error.code(), "download_destination_name_invalid");
}

#[cfg(unix)]
#[test]
fn symlink_destination_directory_is_rejected_before_publish() {
    use std::os::unix::fs::symlink;

    let directory = tempfile::tempdir().expect("tempdir");
    let workspace = directory.path().join("workspace");
    let target = directory.path().join("target");
    let destination = directory.path().join("chosen");
    fs::create_dir_all(&target).expect("target");
    symlink(&target, &destination).expect("destination symlink");

    let plan =
        plan_workspace(&workspace, &destination, "download-000005", "pack.mcaddon").expect("plan");
    ensure_workspace(&plan).expect("workspace");
    fs::write(&plan.payload_path, b"payload").expect("payload");

    let error = finalize_payload(&plan).expect_err("symlink destination must fail closed");
    assert_eq!(error.code(), "download_destination_directory_invalid");
    assert!(!target.join("pack.mcaddon").exists());
}
