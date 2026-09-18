use super::*;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::Arc,
    thread,
    time::{Duration, Instant},
};

struct ResponseSpec {
    status: &'static str,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
    header_delay: Duration,
    chunk_size: usize,
    chunk_delay: Duration,
}

impl ResponseSpec {
    fn ok(body: Vec<u8>) -> Self {
        Self {
            status: "200 OK",
            headers: vec![("Content-Length".into(), body.len().to_string())],
            body,
            header_delay: Duration::ZERO,
            chunk_size: usize::MAX,
            chunk_delay: Duration::ZERO,
        }
    }
}

fn spawn_server(responses: Vec<ResponseSpec>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind http fixture");
    let address = listener.local_addr().expect("fixture address");
    thread::spawn(move || {
        for response in responses {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let _ = read_request(&mut stream);
            if response.header_delay > Duration::ZERO {
                thread::sleep(response.header_delay);
            }
            if write_response(&mut stream, response).is_err() {
                return;
            }
        }
    });
    format!("http://{address}")
}

fn read_request(stream: &mut TcpStream) -> std::io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_secs(1)))?;
    let mut buffer = [0_u8; 4096];
    let _ = stream.read(&mut buffer)?;
    Ok(())
}

fn write_response(stream: &mut TcpStream, response: ResponseSpec) -> std::io::Result<()> {
    write!(stream, "HTTP/1.1 {}\r\n", response.status)?;
    for (name, value) in response.headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    write!(stream, "Connection: close\r\n\r\n")?;
    stream.flush()?;

    let chunk_size = response.chunk_size.max(1);
    for chunk in response.body.chunks(chunk_size) {
        stream.write_all(chunk)?;
        stream.flush()?;
        if response.chunk_delay > Duration::ZERO {
            thread::sleep(response.chunk_delay);
        }
    }
    Ok(())
}

fn source(url: String) -> DownloadSourceRef {
    DownloadSourceRef {
        transport: PUBLIC_HTTPS_TRANSPORT_KEY.into(),
        resource_id: url,
    }
}

fn test_policy() -> HttpTransportPolicy {
    HttpTransportPolicy {
        connect_timeout: Duration::from_secs(1),
        read_timeout: Duration::from_secs(1),
        overall_timeout: Duration::from_secs(5),
        max_redirects: 3,
        max_response_bytes: 2 * 1024 * 1024,
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
            "HTTP fixture timed out"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn expect_open_error(
    result: Result<DownloadTransportStream, DownloadTransportFailure>,
    message: &str,
) -> DownloadTransportFailure {
    match result {
        Err(error) => error,
        Ok(_) => panic!("{message}"),
    }
}

#[test]
fn production_http_transport_rejects_plain_http() {
    let transport = HttpTransport::new(test_policy()).expect("transport");
    let error = expect_open_error(
        transport.open(&source("http://example.com/file.mcpack".into())),
        "plain HTTP must be rejected",
    );
    assert_eq!(error.code, "download_http_scheme_rejected");
    assert!(!error.retryable);
}

#[test]
fn public_http_job_rejects_persisted_query_string() {
    let transport = HttpTransport::new(test_policy()).expect("transport");
    let error = expect_open_error(
        transport.open(&source(
            "https://example.com/file.mcpack?token=secret".into(),
        )),
        "query strings must use a runtime resolver",
    );
    assert_eq!(error.code, "download_http_query_not_persistable");
}

#[test]
fn local_http_fixture_streams_successfully() {
    let payload = b"searchnow-http-fixture".repeat(1024);
    let base = spawn_server(vec![ResponseSpec::ok(payload.clone())]);
    let transport = HttpTransport::new_test_http(test_policy()).expect("transport");
    let mut stream = transport
        .open(&source(format!("{base}/file.mcpack")))
        .expect("open HTTP fixture");
    let mut output = Vec::new();
    stream
        .reader
        .read_to_end(&mut output)
        .expect("read fixture");
    assert_eq!(stream.total_bytes, Some(payload.len() as u64));
    assert_eq!(output, payload);
}

#[test]
fn relative_redirect_is_followed_within_limit() {
    let payload = b"redirected".repeat(100);
    let base = spawn_server(vec![
        ResponseSpec {
            status: "302 Found",
            headers: vec![
                ("Location".into(), "/final".into()),
                ("Content-Length".into(), "0".into()),
            ],
            body: Vec::new(),
            header_delay: Duration::ZERO,
            chunk_size: usize::MAX,
            chunk_delay: Duration::ZERO,
        },
        ResponseSpec::ok(payload.clone()),
    ]);
    let transport = HttpTransport::new_test_http(test_policy()).expect("transport");
    let mut stream = transport
        .open(&source(format!("{base}/start")))
        .expect("follow redirect");
    let mut output = Vec::new();
    stream
        .reader
        .read_to_end(&mut output)
        .expect("read redirect");
    assert_eq!(output, payload);
}

#[test]
fn declared_oversized_response_is_rejected_before_streaming() {
    let base = spawn_server(vec![ResponseSpec {
        status: "200 OK",
        headers: vec![("Content-Length".into(), "100".into())],
        body: vec![0_u8; 100],
        header_delay: Duration::ZERO,
        chunk_size: usize::MAX,
        chunk_delay: Duration::ZERO,
    }]);
    let mut policy = test_policy();
    policy.max_response_bytes = 10;
    let transport = HttpTransport::new_test_http(policy).expect("transport");
    let error = expect_open_error(
        transport.open(&source(format!("{base}/large"))),
        "oversized response must fail",
    );
    assert_eq!(error.code, "download_http_response_too_large");
    assert!(!error.retryable);
}

#[test]
fn short_body_with_content_length_surfaces_read_failure() {
    let base = spawn_server(vec![ResponseSpec {
        status: "200 OK",
        headers: vec![("Content-Length".into(), "10".into())],
        body: b"short".to_vec(),
        header_delay: Duration::ZERO,
        chunk_size: usize::MAX,
        chunk_delay: Duration::ZERO,
    }]);
    let transport = HttpTransport::new_test_http(test_policy()).expect("transport");
    let mut stream = transport
        .open(&source(format!("{base}/short")))
        .expect("open short fixture");
    let mut output = Vec::new();
    assert!(stream.reader.read_to_end(&mut output).is_err());
}

#[test]
fn stalled_response_body_times_out_as_retryable() {
    let payload = vec![5_u8; 2];
    let base = spawn_server(vec![ResponseSpec {
        status: "200 OK",
        headers: vec![("Content-Length".into(), payload.len().to_string())],
        body: payload.clone(),
        header_delay: Duration::ZERO,
        chunk_size: 1,
        chunk_delay: Duration::from_millis(2500),
    }]);
    let mut policy = test_policy();
    policy.read_timeout = Duration::from_millis(50);
    policy.overall_timeout = Duration::from_secs(2);
    let directory = tempfile::tempdir().expect("tempdir");
    let mut registry = DownloadTransportRegistry::new();
    registry
        .register(Arc::new(
            HttpTransport::new_test_http(policy).expect("HTTP transport"),
        ))
        .expect("register HTTP transport");
    let runtime = DownloadExecutionRuntime::new(
        DownloadPolicy::default(),
        DownloadStore::new(directory.path().join("state.json")),
        directory.path().join("workspace"),
        directory.path().join("files"),
        registry,
    )
    .expect("runtime");
    let job = runtime
        .queue(DownloadRequest {
            source: source(format!("{base}/stalled-body")),
            display_name: "http-timeout".into(),
            destination_file_name: "http-timeout.mcpack".into(),
            expected_bytes: Some(payload.len() as u64),
            expected_sha256: None,
        })
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
        .expect("failed timeout job");
    let failure = failed.last_error.as_ref().expect("timeout failure");
    assert_eq!(failure.code, "download_transfer_timeout");
    assert!(failure.retryable);
}

#[test]
fn http_executor_cancellation_is_cooperative() {
    let payload = vec![9_u8; 1024 * 1024];
    let base = spawn_server(vec![ResponseSpec {
        status: "200 OK",
        headers: vec![("Content-Length".into(), payload.len().to_string())],
        body: payload.clone(),
        header_delay: Duration::ZERO,
        chunk_size: 16 * 1024,
        chunk_delay: Duration::from_millis(10),
    }]);
    let directory = tempfile::tempdir().expect("tempdir");
    let mut registry = DownloadTransportRegistry::new();
    registry
        .register(Arc::new(
            HttpTransport::new_test_http(test_policy()).expect("HTTP transport"),
        ))
        .expect("register HTTP transport");
    let runtime = DownloadExecutionRuntime::new(
        DownloadPolicy::default(),
        DownloadStore::new(directory.path().join("state.json")),
        directory.path().join("workspace"),
        directory.path().join("files"),
        registry,
    )
    .expect("runtime");
    let job = runtime
        .queue(DownloadRequest {
            source: source(format!("{base}/slow-body")),
            display_name: "http-cancel".into(),
            destination_file_name: "http-cancel.mcpack".into(),
            expected_bytes: Some(payload.len() as u64),
            expected_sha256: None,
        })
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
            .expect("cancelled job")
            .state,
        DownloadJobState::Cancelled
    );
    assert!(!directory.path().join("files/http-cancel.mcpack").exists());
}
