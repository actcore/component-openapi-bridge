//! openapi-bridge — dynamically expose an OpenAPI spec's endpoints as
//! local ACT tools.
//!
//! Each session corresponds to one upstream API: `open-session` takes
//! `spec_url` plus optional default `headers`, the bridge fetches and
//! parses the spec, and subsequent capability calls operate against
//! that session via `std:session-id`. The parsed spec is cached by
//! `spec_url` so multiple sessions targeting the same API share the
//! parse.

#![allow(clippy::all)]

mod cache;
mod request;
mod spec;
mod tools;

use act_types::cbor;
use spec::BridgeConfig;

wit_bindgen::generate!({
    path: "wit",
    world: "component-world",
    generate_all,
});

use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use exports::act::sessions::session_provider as session_exports;
use exports::act::tools::tool_provider as tool_exports;

// ── Per-session state ──────────────────────────────────────────────────────

struct UpstreamSession {
    config: BridgeConfig,
}

thread_local! {
    static SESSIONS: RefCell<HashMap<String, UpstreamSession>> = RefCell::new(HashMap::new());
    static NEXT_ID: Cell<u64> = const { Cell::new(0) };
}

fn alloc_session_id() -> String {
    NEXT_ID.with(|n| {
        let id = n.get();
        n.set(id + 1);
        format!("openapi_{id}")
    })
}

fn snapshot_session(session_id: &str) -> Option<BridgeConfig> {
    SESSIONS.with(|s| s.borrow().get(session_id).map(|u| u.config.clone()))
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn extract_session_id(metadata: &[(String, Vec<u8>)]) -> Option<String> {
    metadata
        .iter()
        .find(|(k, _)| k == "std:session-id")
        .and_then(|(_, v)| {
            ciborium::from_reader::<serde_json::Value, _>(v.as_slice())
                .ok()
                .and_then(|val| match val {
                    serde_json::Value::String(s) => Some(s),
                    _ => None,
                })
        })
}

fn make_error(kind: &str, msg: String) -> tool_exports::Error {
    tool_exports::Error {
        kind: kind.to_string(),
        message: tool_exports::LocalizedString::Plain(msg),
        metadata: vec![],
    }
}

fn invalid_args(msg: impl Into<String>) -> tool_exports::Error {
    make_error(act_types::constants::ERR_INVALID_ARGS, msg.into())
}

fn session_not_found(session_id: &str) -> tool_exports::Error {
    make_error(
        act_types::constants::ERR_SESSION_NOT_FOUND,
        format!("Unknown session-id: {session_id}"),
    )
}

/// Extract the origin (scheme + authority) from a URL.
fn url_origin(url: &str) -> String {
    if let Some((scheme, rest)) = url.split_once("://") {
        let authority = rest.split('/').next().unwrap_or(rest);
        format!("{scheme}://{authority}")
    } else {
        String::new()
    }
}

/// Resolve a server base URL against the spec URL. Relative server URLs
/// are anchored to the spec's origin.
fn resolve_base_url(spec_url: &str, server_url: &str) -> String {
    if server_url.contains("://") {
        server_url.to_string()
    } else {
        format!("{}{}", url_origin(spec_url), server_url)
    }
}

/// Fetch the OpenAPI spec from a URL using wasi-fetch.
async fn fetch_spec(url: &str) -> Result<String, String> {
    let response = wasi_fetch::Client::new()
        .get(url)
        .header(
            "accept",
            "application/json, application/yaml, text/yaml, */*",
        )
        .send()
        .await
        .map_err(|e| format!("Failed to fetch spec: {e}"))?;

    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(format!("Spec fetch returned HTTP {status}"));
    }

    response
        .into_body()
        .text()
        .await
        .map_err(|e| format!("Spec response is not valid UTF-8: {e}"))
}

/// Fetch (or use cached) tools for the given config's spec_url.
async fn get_or_fetch_tools(config: &BridgeConfig) -> Result<Vec<tools::ResolvedTool>, String> {
    if let Some(cached) = cache::get_cached(&config.spec_url) {
        return Ok(cached);
    }

    let body = fetch_spec(&config.spec_url).await?;
    let spec = spec::OpenApiSpec::parse(&body)?;
    let resolved = tools::extract_tools(&spec);

    cache::put_cached(config.spec_url.clone(), spec, resolved.clone());

    Ok(resolved)
}

/// Convert a ResolvedTool to a WIT ToolDefinition.
fn to_wit_tool(tool: &tools::ResolvedTool) -> tool_exports::ToolDefinition {
    let mut metadata = Vec::new();

    if tool.metadata_flags.read_only {
        metadata.push((
            act_types::constants::META_READ_ONLY.to_string(),
            cbor::to_cbor(&true),
        ));
    }
    if tool.metadata_flags.idempotent {
        metadata.push((
            act_types::constants::META_IDEMPOTENT.to_string(),
            cbor::to_cbor(&true),
        ));
    }
    if tool.metadata_flags.destructive {
        metadata.push((
            act_types::constants::META_DESTRUCTIVE.to_string(),
            cbor::to_cbor(&true),
        ));
    }

    let schema = tools::build_parameters_schema(tool);
    let schema_str =
        serde_json::to_string(&schema).unwrap_or_else(|_| r#"{"type":"object"}"#.to_string());

    tool_exports::ToolDefinition {
        name: tool.name.clone(),
        description: tool_exports::LocalizedString::Plain(tool.description.clone()),
        parameters_schema: schema_str,
        metadata,
    }
}

/// Send an HTTP request via wasi-fetch and stream the response back.
async fn send_api_request(
    prepared: request::PreparedRequest,
    writer: &mut wit_bindgen::StreamWriter<tool_exports::ToolEvent>,
) {
    let mut builder = wasi_fetch::Client::new()
        .request(prepared.method, &prepared.url)
        .redirect_limit(0);

    for (name, value) in prepared.headers.iter() {
        if let Ok(v) = value.to_str() {
            builder = builder.header(name.as_str(), v);
        }
    }

    if let Some(body) = prepared.body {
        builder = builder.body(body);
    }

    let response = match builder.send().await {
        Ok(r) => r,
        Err(e) => {
            let _ = writer
                .write_all(vec![tool_exports::ToolEvent::Error(make_error(
                    act_types::constants::ERR_INTERNAL,
                    format!("HTTP error: {e}"),
                ))])
                .await;
            return;
        }
    };

    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    if status >= 400 {
        let body = response.into_body().text().await.unwrap_or_default();
        let _ = writer
            .write_all(vec![tool_exports::ToolEvent::Error(make_error(
                act_types::constants::ERR_INTERNAL,
                format!("HTTP {status}: {body}"),
            ))])
            .await;
        return;
    }

    let mut body = response.into_body();
    while let Some(chunk) = body.chunk().await {
        let _ = writer
            .write_all(vec![tool_exports::ToolEvent::Content(
                tool_exports::ContentPart {
                    data: chunk.to_vec(),
                    mime_type: content_type.clone(),
                    metadata: vec![],
                },
            )])
            .await;
    }
}

// ── Component entry point ──────────────────────────────────────────────────

struct OpenApiBridge;

export!(OpenApiBridge);

// ── tool-provider ──────────────────────────────────────────────────────────

impl tool_exports::Guest for OpenApiBridge {
    async fn list_tools(
        metadata: Vec<(String, Vec<u8>)>,
    ) -> Result<tool_exports::ListToolsResponse, tool_exports::Error> {
        let session_id = match extract_session_id(&metadata) {
            Some(id) => id,
            None => {
                return Ok(tool_exports::ListToolsResponse {
                    metadata: vec![],
                    tools: vec![],
                });
            }
        };

        let config = match snapshot_session(&session_id) {
            Some(c) => c,
            None => return Err(session_not_found(&session_id)),
        };

        let resolved = get_or_fetch_tools(&config)
            .await
            .map_err(|e| make_error(act_types::constants::ERR_INTERNAL, e))?;

        let tool_defs: Vec<tool_exports::ToolDefinition> =
            resolved.iter().map(to_wit_tool).collect();

        Ok(tool_exports::ListToolsResponse {
            metadata: vec![],
            tools: tool_defs,
        })
    }

    async fn call_tool(
        name: String,
        arguments: Vec<u8>,
        metadata: Vec<(String, Vec<u8>)>,
    ) -> tool_exports::ToolResult {
        let session_id = match extract_session_id(&metadata) {
            Some(id) => id,
            None => {
                return tool_exports::ToolResult::Immediate(vec![tool_exports::ToolEvent::Error(
                    invalid_args("Missing required metadata key std:session-id"),
                )]);
            }
        };

        let config = match snapshot_session(&session_id) {
            Some(c) => c,
            None => {
                return tool_exports::ToolResult::Immediate(vec![tool_exports::ToolEvent::Error(
                    session_not_found(&session_id),
                )]);
            }
        };

        let (mut writer, reader) = wit_stream::new::<tool_exports::ToolEvent>();

        wit_bindgen::spawn(async move {
            // Resolve the operation. open-session pre-fetched the spec,
            // so this should hit the cache; fall through to a refetch if
            // the cache was evicted.
            let tool = match cache::get_cached_tool(&config.spec_url, &name) {
                Some(t) => t,
                None => match get_or_fetch_tools(&config).await {
                    Ok(_) => match cache::get_cached_tool(&config.spec_url, &name) {
                        Some(t) => t,
                        None => {
                            let _ = writer
                                .write_all(vec![tool_exports::ToolEvent::Error(make_error(
                                    act_types::constants::ERR_NOT_FOUND,
                                    format!("Tool '{name}' not found in spec"),
                                ))])
                                .await;
                            return;
                        }
                    },
                    Err(e) => {
                        let _ = writer
                            .write_all(vec![tool_exports::ToolEvent::Error(make_error(
                                act_types::constants::ERR_INTERNAL,
                                e,
                            ))])
                            .await;
                        return;
                    }
                },
            };

            // Decode arguments from CBOR.
            let args: serde_json::Value = if arguments.is_empty() {
                serde_json::json!({})
            } else {
                match cbor::from_cbor(&arguments) {
                    Ok(v) => v,
                    Err(e) => {
                        let _ = writer
                            .write_all(vec![tool_exports::ToolEvent::Error(invalid_args(format!(
                                "Invalid arguments: {e}"
                            )))])
                            .await;
                        return;
                    }
                }
            };

            // Per-call header overrides (callers can still pass per-tool
            // headers via metadata).
            let call_headers = request::extract_call_headers(&metadata);

            let raw_base = cache::get_base_url(&config.spec_url).unwrap_or_default();
            let base_url = resolve_base_url(&config.spec_url, &raw_base);

            let prepared = match request::build_request(
                &tool,
                &args,
                &base_url,
                &config.headers,
                &call_headers,
            ) {
                Ok(r) => r,
                Err(e) => {
                    let _ = writer
                        .write_all(vec![tool_exports::ToolEvent::Error(invalid_args(e))])
                        .await;
                    return;
                }
            };

            send_api_request(prepared, &mut writer).await;
        });

        tool_exports::ToolResult::Streaming(reader)
    }
}

// ── session-provider ───────────────────────────────────────────────────────

impl session_exports::Guest for OpenApiBridge {
    async fn get_open_session_args_schema(
        _metadata: Vec<(String, Vec<u8>)>,
    ) -> Result<String, session_exports::Error> {
        let schema = schemars::schema_for!(BridgeConfig);
        serde_json::to_string(&schema).map_err(|e| session_exports::Error {
            kind: act_types::constants::ERR_INTERNAL.to_string(),
            message: tool_exports::LocalizedString::Plain(format!(
                "Schema serialization failed: {e}"
            )),
            metadata: vec![],
        })
    }

    async fn open_session(
        args: Vec<(String, Vec<u8>)>,
        _metadata: Vec<(String, Vec<u8>)>,
    ) -> Result<session_exports::Session, session_exports::Error> {
        let mut json_map = serde_json::Map::with_capacity(args.len());
        for (k, v) in &args {
            if let Ok(val) = ciborium::from_reader::<serde_json::Value, _>(v.as_slice()) {
                json_map.insert(k.clone(), val);
            }
        }
        let config: BridgeConfig = serde_json::from_value(serde_json::Value::Object(json_map))
            .map_err(|e| session_exports::Error {
                kind: act_types::constants::ERR_INVALID_ARGS.to_string(),
                message: tool_exports::LocalizedString::Plain(format!(
                    "Invalid open-session args: {e}"
                )),
                metadata: vec![],
            })?;

        // Pre-fetch the spec so list-tools is cheap and connect / parse
        // failures surface at open time (per ACT-SESSIONS §2.2).
        get_or_fetch_tools(&config)
            .await
            .map_err(|e| session_exports::Error {
                kind: act_types::constants::ERR_INTERNAL.to_string(),
                message: tool_exports::LocalizedString::Plain(e),
                metadata: vec![],
            })?;

        let id = alloc_session_id();
        SESSIONS.with(|s| {
            s.borrow_mut()
                .insert(id.clone(), UpstreamSession { config });
        });

        Ok(session_exports::Session {
            id,
            metadata: vec![],
        })
    }

    fn close_session(session_id: String) {
        SESSIONS.with(|s| {
            s.borrow_mut().remove(&session_id);
        });
    }
}
