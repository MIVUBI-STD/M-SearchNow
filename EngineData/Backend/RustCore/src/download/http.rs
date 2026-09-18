use super::{
    DownloadSourceRef, DownloadTransport, DownloadTransportFailure, DownloadTransportStream,
};
use crate::error::{BackendError, BackendResult};
use std::{
    io::{self, Read},
    time::{Duration, Instant},
};
use url::Url;

pub const PUBLIC_HTTPS_TRANSPORT_KEY: &str = "https-public";
const HARD_MAX_RESPONSE_BYTES: u64 = 32 * 1024 * 1024 * 1024;
const HARD_MAX_REDIRECTS: u32 = 10;
const MAX_RUNTIME_HEADERS: usize = 32;
const MAX_HEADER_NAME_BYTES: usize = 128;
const MAX_HEADER_VALUE_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HttpTransportPolicy {
    pub connect_timeout: Duration,
    pub read_timeout: Duration,
    pub overall_timeout: Duration,
    pub max_redirects: u32,
    pub max_response_bytes: u64,
}

impl Default for HttpTransportPolicy {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            read_timeout: Duration::from_secs(15),
            overall_timeout: Duration::from_secs(30 * 60),
            max_redirects: 5,
            max_response_bytes: 8 * 1024 * 1024 * 1024,
        }
    }
}

impl HttpTransportPolicy {
    pub fn validate(self) -> BackendResult<()> {
        if self.connect_timeout.is_zero()
            || self.read_timeout.is_zero()
            || self.overall_timeout.is_zero()
            || self.connect_timeout > self.overall_timeout
            || self.read_timeout > self.overall_timeout
            || self.max_redirects > HARD_MAX_REDIRECTS
            || self.max_response_bytes == 0
            || self.max_response_bytes > HARD_MAX_RESPONSE_BYTES
        {
            return Err(BackendError::new(
                "download_http_policy_invalid",
                "HTTP transport timeout, redirect, or response-size policy is invalid.",
            ));
        }
        Ok(())
    }
}

pub fn public_https_download_source(
    resource_url: impl Into<String>,
) -> BackendResult<DownloadSourceRef> {
    let resource_id = resource_url.into();
    parse_source_url(&resource_id, false).map_err(|_| {
        BackendError::new(
            "download_http_public_source_invalid",
            "Public HTTPS resource identity is invalid or contains runtime credential material.",
        )
    })?;
    Ok(DownloadSourceRef {
        transport: PUBLIC_HTTPS_TRANSPORT_KEY.to_string(),
        resource_id,
    })
}

pub(crate) struct RuntimeHttpHeader {
    name: String,
    value: String,
}

impl RuntimeHttpHeader {
    pub(crate) fn new(
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, DownloadTransportFailure> {
        let header = Self {
            name: name.into(),
            value: value.into(),
        };
        header.validate()?;
        Ok(header)
    }

    fn validate(&self) -> Result<(), DownloadTransportFailure> {
        let valid_name = !self.name.is_empty()
            && self.name.len() <= MAX_HEADER_NAME_BYTES
            && self
                .name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-');
        let forbidden_name = [
            "host",
            "content-length",
            "connection",
            "transfer-encoding",
            "proxy-authorization",
            "range",
        ]
        .iter()
        .any(|name| self.name.eq_ignore_ascii_case(name));
        let valid_value =
            self.value.len() <= MAX_HEADER_VALUE_BYTES && !self.value.chars().any(char::is_control);

        if !valid_name || forbidden_name || !valid_value {
            return Err(DownloadTransportFailure::new(
                "download_http_runtime_header_invalid",
                "Resolved runtime HTTP headers contain an unsupported name or value.",
                false,
            ));
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct HttpTransport {
    agent: ureq::Agent,
    policy: HttpTransportPolicy,
    allow_plain_http: bool,
}

impl HttpTransport {
    pub fn new(policy: HttpTransportPolicy) -> BackendResult<Self> {
        Self::build(policy, false)
    }

    fn build(policy: HttpTransportPolicy, allow_plain_http: bool) -> BackendResult<Self> {
        policy.validate()?;
        let agent = ureq::AgentBuilder::new()
            .redirects(0)
            .timeout_connect(policy.connect_timeout)
            .timeout_read(policy.read_timeout)
            .timeout_write(policy.connect_timeout)
            .build();
        Ok(Self {
            agent,
            policy,
            allow_plain_http,
        })
    }

    #[cfg(test)]
    pub(crate) fn new_test_http(policy: HttpTransportPolicy) -> BackendResult<Self> {
        Self::build(policy, true)
    }

    pub(crate) fn open_runtime_request(
        &self,
        url: &str,
        headers: &[RuntimeHttpHeader],
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        if headers.len() > MAX_RUNTIME_HEADERS {
            return Err(DownloadTransportFailure::new(
                "download_http_runtime_headers_too_many",
                "Resolved runtime HTTP request contains too many headers.",
                false,
            ));
        }
        let parsed = Url::parse(url).map_err(|_| {
            DownloadTransportFailure::new(
                "download_http_runtime_url_invalid",
                "Resolved runtime HTTP URL is invalid.",
                false,
            )
        })?;
        validate_runtime_url(&parsed, self.allow_plain_http)?;
        self.open_parsed_url(parsed, headers, true, 0)
    }

    pub(crate) fn open_runtime_request_from(
        &self,
        url: &str,
        headers: &[RuntimeHttpHeader],
        offset: u64,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        if headers.len() > MAX_RUNTIME_HEADERS {
            return Err(DownloadTransportFailure::new(
                "download_http_runtime_headers_too_many",
                "Resolved runtime HTTP request contains too many headers.",
                false,
            ));
        }
        let parsed = Url::parse(url).map_err(|_| {
            DownloadTransportFailure::new(
                "download_http_runtime_url_invalid",
                "Resolved runtime HTTP URL is invalid.",
                false,
            )
        })?;
        validate_runtime_url(&parsed, self.allow_plain_http)?;
        self.open_parsed_url(parsed, headers, true, offset)
    }

    fn open_url(
        &self,
        source: &DownloadSourceRef,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        if source.transport != PUBLIC_HTTPS_TRANSPORT_KEY {
            return Err(DownloadTransportFailure::new(
                "download_http_transport_key_invalid",
                "HTTP transport received a job for a different transport key.",
                false,
            ));
        }

        let current = parse_source_url(&source.resource_id, self.allow_plain_http)?;
        self.open_parsed_url(current, &[], false, 0)
    }

    fn open_url_from(
        &self,
        source: &DownloadSourceRef,
        offset: u64,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        if source.transport != PUBLIC_HTTPS_TRANSPORT_KEY {
            return Err(DownloadTransportFailure::new(
                "download_http_transport_key_invalid",
                "HTTP transport received a job for a different transport key.",
                false,
            ));
        }
        let current = parse_source_url(&source.resource_id, self.allow_plain_http)?;
        self.open_parsed_url(current, &[], false, offset)
    }

    fn open_parsed_url(
        &self,
        mut current: Url,
        headers: &[RuntimeHttpHeader],
        sensitive_runtime_material: bool,
        offset: u64,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        let started_at = Instant::now();
        let mut redirects = 0_u32;

        loop {
            ensure_overall_deadline(started_at, self.policy.overall_timeout)?;
            let mut request = self.agent.get(current.as_str());
            for header in headers {
                request = request.set(&header.name, &header.value);
            }
            if offset > 0 {
                request = request.set("Range", &format!("bytes={offset}-"));
            }
            let response = match request.call() {
                Ok(response) => response,
                Err(error) => {
                    return Err(map_request_error(error, sensitive_runtime_material));
                }
            };
            ensure_overall_deadline(started_at, self.policy.overall_timeout)?;
            let status = response.status();

            if matches!(status, 301 | 302 | 303 | 307 | 308) {
                if redirects >= self.policy.max_redirects {
                    return Err(DownloadTransportFailure::new(
                        "download_http_redirect_limit",
                        "HTTP response exceeded SearchNow's redirect limit.",
                        false,
                    ));
                }
                let location = response.header("Location").ok_or_else(|| {
                    DownloadTransportFailure::new(
                        "download_http_redirect_missing_location",
                        "HTTP redirect did not provide a Location header.",
                        false,
                    )
                })?;
                let next = current.join(location).map_err(|_| {
                    DownloadTransportFailure::new(
                        "download_http_redirect_invalid",
                        "HTTP redirect target is invalid.",
                        false,
                    )
                })?;
                validate_runtime_url(&next, self.allow_plain_http)?;
                if sensitive_runtime_material
                    && !headers.is_empty()
                    && !same_origin(&current, &next)
                {
                    return Err(DownloadTransportFailure::new(
                        "download_http_sensitive_redirect_rejected",
                        "Resolved authenticated HTTP requests cannot redirect credentials across origins.",
                        false,
                    ));
                }
                current = next;
                redirects += 1;
                continue;
            }

            if offset > 0 && status != 206 {
                return Err(DownloadTransportFailure::new(
                    "download_http_resume_unsupported",
                    "The server did not honor the requested byte range.",
                    false,
                ));
            }
            if !(200..300).contains(&status) {
                return Err(status_failure(status));
            }

            let total_bytes = if offset > 0 {
                parse_content_range_total(&response, offset)?
            } else {
                parse_content_length(&response)?
            };
            if total_bytes.is_some_and(|value| value > self.policy.max_response_bytes) {
                return Err(DownloadTransportFailure::new(
                    "download_http_response_too_large",
                    format!(
                        "HTTP response exceeds the configured {} byte safety limit.",
                        self.policy.max_response_bytes
                    ),
                    false,
                ));
            }

            let response_limit = self.policy.max_response_bytes.saturating_sub(offset);
            let limited = ResponseLimitReader::new(response.into_reader(), response_limit);
            let reader =
                OverallDeadlineReader::new(limited, started_at, self.policy.overall_timeout);
            return Ok(DownloadTransportStream {
                reader: Box::new(reader),
                total_bytes,
            });
        }
    }
}

impl DownloadTransport for HttpTransport {
    fn key(&self) -> &str {
        PUBLIC_HTTPS_TRANSPORT_KEY
    }

    fn open(
        &self,
        source: &DownloadSourceRef,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        self.open_url(source)
    }

    fn open_from(
        &self,
        source: &DownloadSourceRef,
        offset: u64,
    ) -> Result<DownloadTransportStream, DownloadTransportFailure> {
        self.open_url_from(source, offset)
    }
}

fn parse_source_url(value: &str, allow_plain_http: bool) -> Result<Url, DownloadTransportFailure> {
    let url = Url::parse(value).map_err(|error| {
        DownloadTransportFailure::new(
            "download_http_url_invalid",
            format!("Public HTTP resource URL is invalid: {error}"),
            false,
        )
    })?;
    validate_runtime_url(&url, allow_plain_http)?;
    if url.query().is_some() {
        return Err(DownloadTransportFailure::new(
            "download_http_query_not_persistable",
            "Public HTTPS download jobs cannot persist query strings; use a runtime resolver for signed or authenticated URLs.",
            false,
        ));
    }
    Ok(url)
}

fn validate_runtime_url(url: &Url, allow_plain_http: bool) -> Result<(), DownloadTransportFailure> {
    let scheme_allowed = url.scheme() == "https" || (allow_plain_http && url.scheme() == "http");
    if !scheme_allowed {
        return Err(DownloadTransportFailure::new(
            "download_http_scheme_rejected",
            "Public HTTP transport accepts HTTPS URLs only.",
            false,
        ));
    }
    if url.host_str().is_none() {
        return Err(DownloadTransportFailure::new(
            "download_http_host_missing",
            "HTTP resource URL must include a host.",
            false,
        ));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(DownloadTransportFailure::new(
            "download_http_userinfo_rejected",
            "HTTP resource URLs cannot embed usernames or passwords.",
            false,
        ));
    }
    if url.fragment().is_some() {
        return Err(DownloadTransportFailure::new(
            "download_http_fragment_rejected",
            "HTTP resource URLs cannot contain fragments.",
            false,
        ));
    }
    Ok(())
}

fn parse_content_length(
    response: &ureq::Response,
) -> Result<Option<u64>, DownloadTransportFailure> {
    let Some(value) = response.header("Content-Length") else {
        return Ok(None);
    };
    value.parse::<u64>().map(Some).map_err(|error| {
        DownloadTransportFailure::new(
            "download_http_content_length_invalid",
            format!("HTTP Content-Length is invalid: {error}"),
            false,
        )
    })
}

fn parse_content_range_total(
    response: &ureq::Response,
    expected_start: u64,
) -> Result<Option<u64>, DownloadTransportFailure> {
    let value = response.header("Content-Range").ok_or_else(|| {
        DownloadTransportFailure::new(
            "download_http_content_range_missing",
            "Partial HTTP response did not include Content-Range.",
            false,
        )
    })?;
    let range = value.strip_prefix("bytes ").ok_or_else(|| {
        DownloadTransportFailure::new(
            "download_http_content_range_invalid",
            "Partial HTTP response included an invalid Content-Range.",
            false,
        )
    })?;
    let (bounds, total) = range.split_once('/').ok_or_else(|| {
        DownloadTransportFailure::new(
            "download_http_content_range_invalid",
            "Partial HTTP response included an invalid Content-Range.",
            false,
        )
    })?;
    let (start, _) = bounds.split_once('-').ok_or_else(|| {
        DownloadTransportFailure::new(
            "download_http_content_range_invalid",
            "Partial HTTP response included an invalid Content-Range.",
            false,
        )
    })?;
    let start = start.parse::<u64>().map_err(|_| {
        DownloadTransportFailure::new(
            "download_http_content_range_invalid",
            "Partial HTTP response included an invalid Content-Range.",
            false,
        )
    })?;
    if start != expected_start {
        return Err(DownloadTransportFailure::new(
            "download_http_content_range_mismatch",
            "Partial HTTP response started at an unexpected byte offset.",
            false,
        ));
    }
    if total == "*" {
        return Ok(None);
    }
    total.parse::<u64>().map(Some).map_err(|_| {
        DownloadTransportFailure::new(
            "download_http_content_range_invalid",
            "Partial HTTP response included an invalid total size.",
            false,
        )
    })
}

fn ensure_overall_deadline(
    started_at: Instant,
    timeout: Duration,
) -> Result<(), DownloadTransportFailure> {
    if started_at.elapsed() >= timeout {
        Err(DownloadTransportFailure::new(
            "download_http_overall_timeout",
            "HTTP download exceeded SearchNow's overall transfer deadline.",
            true,
        ))
    } else {
        Ok(())
    }
}

fn map_request_error(
    error: ureq::Error,
    sensitive_runtime_material: bool,
) -> DownloadTransportFailure {
    match error {
        ureq::Error::Status(status, _) => status_failure(status),
        ureq::Error::Transport(error) if !sensitive_runtime_material => {
            DownloadTransportFailure::new(
                "download_http_request_failed",
                format!("HTTP request failed: {error}"),
                true,
            )
        }
        ureq::Error::Transport(_) => DownloadTransportFailure::new(
            "download_http_request_failed",
            "Resolved provider HTTP request failed without exposing runtime request material.",
            true,
        ),
    }
}

fn status_failure(status: u16) -> DownloadTransportFailure {
    let retryable = matches!(status, 408 | 425 | 429 | 500 | 502 | 503 | 504);
    DownloadTransportFailure::new(
        "download_http_status_error",
        format!("HTTP server returned status {status}."),
        retryable,
    )
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

struct OverallDeadlineReader<R> {
    inner: R,
    started_at: Instant,
    timeout: Duration,
}

impl<R> OverallDeadlineReader<R> {
    fn new(inner: R, started_at: Instant, timeout: Duration) -> Self {
        Self {
            inner,
            started_at,
            timeout,
        }
    }

    fn deadline_error() -> io::Error {
        io::Error::new(
            io::ErrorKind::TimedOut,
            "HTTP download exceeded SearchNow's overall transfer deadline.",
        )
    }
}

impl<R: Read> Read for OverallDeadlineReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if self.started_at.elapsed() >= self.timeout {
            return Err(Self::deadline_error());
        }
        let read = self.inner.read(buffer)?;
        if self.started_at.elapsed() >= self.timeout {
            return Err(Self::deadline_error());
        }
        Ok(read)
    }
}

struct ResponseLimitReader<R> {
    inner: R,
    remaining: u64,
}

impl<R> ResponseLimitReader<R> {
    fn new(inner: R, limit: u64) -> Self {
        Self {
            inner,
            remaining: limit,
        }
    }
}

impl<R: Read> Read for ResponseLimitReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            let mut probe = [0_u8; 1];
            return match self.inner.read(&mut probe) {
                Ok(0) => Ok(0),
                Ok(_) => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "HTTP response exceeded the configured download safety limit.",
                )),
                Err(error) => Err(error),
            };
        }

        let allowed = buffer.len().min(self.remaining as usize);
        let read = self.inner.read(&mut buffer[..allowed])?;
        self.remaining = self.remaining.saturating_sub(read as u64);
        Ok(read)
    }
}
