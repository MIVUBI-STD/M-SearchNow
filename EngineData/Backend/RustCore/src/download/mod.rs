mod executor;
mod http;
mod manager;
mod model;
mod resolver;
mod store;
mod transport;
mod workspace;

pub(crate) use executor::{default_download_paths, DownloadExecutionRuntime};
pub(crate) use http::{
    public_https_download_source, HttpTransport, HttpTransportPolicy, PUBLIC_HTTPS_TRANSPORT_KEY,
};
pub(crate) use manager::DownloadManager;
pub use model::*;
pub use resolver::{
    ProviderResolveFailure, ProviderResourceRef, ResolvedResource, ResourceResolver,
    ResourceResolverRegistry,
};
pub(crate) use resolver::{
    provider_download_source, ProviderResolvedTransport, RESOLVED_PROVIDER_TRANSPORT_KEY,
};
pub(crate) use store::DownloadStore;
pub(crate) use transport::{
    DownloadTransport, DownloadTransportFailure, DownloadTransportRegistry,
    DownloadTransportStream, LocalFileTransport,
};
pub(crate) use workspace::{
    cleanup_finalization_stage, cleanup_workspace, ensure_workspace, finalization_stage_path,
    finalize_payload, finalized_file_matches, plan_workspace, prepare_payload_file,
    validate_destination_file_name, DownloadWorkspacePlan,
};

#[cfg(test)]
mod executor_tests;
#[cfg(test)]
mod http_tests;
#[cfg(test)]
mod quality_tests;
#[cfg(test)]
mod resolver_tests;
#[cfg(test)]
mod tests;
