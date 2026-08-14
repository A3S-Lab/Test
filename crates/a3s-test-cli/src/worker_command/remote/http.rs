use std::{future::IntoFuture, sync::Arc, time::Duration};

use a3s_test_worker::{RemoteArtifactRequest, RemoteWorkerRequest, RemoteWorkerService};
use anyhow::{Context, Result};
use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{header, HeaderMap, HeaderValue, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use serde::{de::DeserializeOwned, Serialize};
use tokio::{
    net::TcpListener,
    sync::{OwnedSemaphorePermit, Semaphore},
};
use tokio_util::sync::CancellationToken;

const MAX_CONCURRENT_HTTP_REQUESTS: usize = 4;

#[derive(Clone)]
struct HttpState {
    service: RemoteWorkerService,
    authorization: Arc<[u8]>,
    max_request_bytes: usize,
    request_body_timeout: Duration,
    request_slots: Arc<Semaphore>,
}

#[derive(Serialize)]
struct TransportError {
    code: &'static str,
    message: &'static str,
}

pub(super) async fn serve(
    listener: TcpListener,
    service: RemoteWorkerService,
    authorization: Arc<[u8]>,
    shutdown: CancellationToken,
    request_body_timeout: Duration,
    cleanup_timeout: Duration,
) -> Result<()> {
    let max_request_bytes = usize::try_from(service.descriptor().limits.max_request_bytes)
        .context("remote request limit does not fit this platform")?;
    let state = HttpState {
        service,
        authorization,
        max_request_bytes,
        request_body_timeout,
        request_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_HTTP_REQUESTS)),
    };
    let application = Router::new()
        .route("/v1/worker", post(handle_worker_request))
        .route("/v1/artifacts", post(handle_artifact_request))
        .with_state(state);
    let graceful_shutdown = shutdown.clone();
    let server = axum::serve(listener, application)
        .with_graceful_shutdown(async move {
            graceful_shutdown.cancelled().await;
        })
        .into_future();
    tokio::pin!(server);
    tokio::select! {
        result = &mut server => {
            result.context("remote worker HTTP server failed")
        }
        _ = shutdown.cancelled() => {
            match tokio::time::timeout(cleanup_timeout, &mut server).await {
                Ok(result) => result.context("remote worker HTTP server failed during shutdown"),
                Err(_) => anyhow::bail!("remote worker HTTP shutdown exceeded its cleanup bound"),
            }
        }
    }
}

async fn handle_worker_request(State(state): State<HttpState>, request: Request<Body>) -> Response {
    let (request, _request_slot) =
        match decode_request::<RemoteWorkerRequest>(&state, request).await {
            Ok(request) => request,
            Err(response) => return response,
        };
    no_store(Json(state.service.handle(request).await).into_response())
}

async fn handle_artifact_request(
    State(state): State<HttpState>,
    request: Request<Body>,
) -> Response {
    let (request, _request_slot) =
        match decode_request::<RemoteArtifactRequest>(&state, request).await {
            Ok(request) => request,
            Err(response) => return response,
        };
    no_store(Json(state.service.handle_artifact(request).await).into_response())
}

async fn decode_request<T: DeserializeOwned>(
    state: &HttpState,
    request: Request<Body>,
) -> Result<(T, OwnedSemaphorePermit), Response> {
    if !authorization_matches(request.headers(), &state.authorization) {
        return Err(transport_error(
            StatusCode::UNAUTHORIZED,
            "test.worker.remote.transport_unauthorized",
            "request did not provide the required transport authorization",
        ));
    }
    if !content_type_is_json(request.headers()) {
        return Err(transport_error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "test.worker.remote.transport_content_type_invalid",
            "remote worker requests require application/json",
        ));
    }
    let request_slot = match Arc::clone(&state.request_slots).try_acquire_owned() {
        Ok(slot) => slot,
        Err(_) => {
            return Err(transport_error(
                StatusCode::TOO_MANY_REQUESTS,
                "test.worker.remote.transport_busy",
                "remote worker HTTP admission is at its concurrency bound",
            ));
        }
    };
    let body = match tokio::time::timeout(
        state.request_body_timeout,
        to_bytes(request.into_body(), state.max_request_bytes),
    )
    .await
    {
        Ok(Ok(body)) => body,
        Ok(Err(_)) => {
            return Err(transport_error(
                StatusCode::PAYLOAD_TOO_LARGE,
                "test.worker.remote.transport_request_too_large",
                "remote worker request exceeds the HTTP body limit",
            ));
        }
        Err(_) => {
            return Err(transport_error(
                StatusCode::REQUEST_TIMEOUT,
                "test.worker.remote.transport_request_timeout",
                "remote worker request body exceeded its read deadline",
            ));
        }
    };
    let request = match serde_json::from_slice::<T>(&body) {
        Ok(request) => request,
        Err(_) => {
            return Err(transport_error(
                StatusCode::BAD_REQUEST,
                "test.worker.remote.transport_json_invalid",
                "remote worker request is not valid strict JSON",
            ));
        }
    };
    Ok((request, request_slot))
}

fn authorization_matches(headers: &HeaderMap, expected: &[u8]) -> bool {
    let mut values = headers.get_all(header::AUTHORIZATION).iter();
    let Some(actual) = values.next() else {
        return false;
    };
    if values.next().is_some() {
        return false;
    }
    let actual = actual.as_bytes();
    let mut difference = actual.len() ^ expected.len();
    for (index, expected_byte) in expected.iter().copied().enumerate() {
        difference |= usize::from(actual.get(index).copied().unwrap_or_default() ^ expected_byte);
    }
    difference == 0
}

fn content_type_is_json(headers: &HeaderMap) -> bool {
    let mut values = headers.get_all(header::CONTENT_TYPE).iter();
    let Some(value) = values.next() else {
        return false;
    };
    if values.next().is_some() {
        return false;
    }
    value
        .to_str()
        .ok()
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("application/json"))
}

fn transport_error(status: StatusCode, code: &'static str, message: &'static str) -> Response {
    no_store((status, Json(TransportError { code, message })).into_response())
}

fn no_store(mut response: Response) -> Response {
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    response
}
