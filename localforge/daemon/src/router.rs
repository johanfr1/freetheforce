//! JSON-RPC method router

use crate::api::types::*;
use crate::config::{ConfigStore, json_to_toml, toml_to_json};
use crate::entitlements::{Grant, GrantStore, TrustStore};
use crate::identity::{IdentityStore, Keypair};
use crate::logging::LogWriter;
use crate::platform::paths::DataDir;
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;

/// Daemon context containing all stores and state
pub struct DaemonContext {
    pub data_dir: DataDir,
    pub identity_store: IdentityStore,
    pub grant_store: GrantStore,
    pub trust_store: TrustStore,
    pub config_store: ConfigStore,
    pub log_writer: Arc<LogWriter>,
    pub start_time: Instant,
    pub version: String,
}

impl DaemonContext {
    /// Create a new daemon context
    pub fn new(data_dir: DataDir) -> Self {
        let log_writer = Arc::new(LogWriter::new(data_dir.clone()));

        Self {
            identity_store: IdentityStore::new(data_dir.clone()),
            grant_store: GrantStore::new(data_dir.clone()),
            trust_store: TrustStore::new(data_dir.clone()),
            config_store: ConfigStore::new(data_dir.clone()),
            log_writer,
            data_dir,
            start_time: Instant::now(),
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Route a JSON-RPC request to the appropriate handler
pub fn route(ctx: &DaemonContext, request: &JsonRpcRequest) -> JsonRpcResponse {
    let result = match request.method.as_str() {
        "status" => handle_status(ctx),
        "identity.get" => handle_identity_get(ctx),
        "identity.init" => handle_identity_init(ctx),
        "identity.sign" => handle_identity_sign(ctx, &request.params),
        "identity.setAlias" => handle_identity_set_alias(ctx, &request.params),
        "identity.export" => handle_identity_export(ctx),
        "identity.import" => handle_identity_import(ctx, &request.params),
        "entitlements.can" => handle_entitlements_can(ctx, &request.params),
        "entitlements.list" => handle_entitlements_list(ctx),
        "entitlements.add" => handle_entitlements_add(ctx, &request.params),
        "entitlements.remove" => handle_entitlements_remove(ctx, &request.params),
        "config.get" => handle_config_get(ctx, &request.params),
        "config.set" => handle_config_set(ctx, &request.params),
        "config.list" => handle_config_list(ctx, &request.params),
        "config.reset" => handle_config_reset(ctx, &request.params),
        _ => Err(JsonRpcError::method_not_found(&request.method)),
    };

    match result {
        Ok(value) => JsonRpcResponse::success(request.id.clone(), value),
        Err(error) => JsonRpcResponse::error(request.id.clone(), error),
    }
}

// Handler implementations

fn handle_status(ctx: &DaemonContext) -> Result<Value, JsonRpcError> {
    let identity = ctx
        .identity_store
        .load_identity()
        .ok()
        .map(|id| id.display_key());

    let (active, expired) = ctx
        .grant_store
        .count_by_status()
        .unwrap_or((0, 0));

    let response = StatusResponse {
        version: ctx.version.clone(),
        status: "running".to_string(),
        uptime_seconds: ctx.start_time.elapsed().as_secs(),
        identity,
        grants_active: active,
        grants_expired: expired,
    };

    Ok(serde_json::to_value(response).unwrap())
}

fn handle_identity_get(ctx: &DaemonContext) -> Result<Value, JsonRpcError> {
    let identity = ctx
        .identity_store
        .load_identity()
        .map_err(|_| JsonRpcError::identity_not_initialized())?;

    let response = IdentityGetResponse {
        public_key: identity.public_key,
        created_at: identity.created_at.to_rfc3339(),
        alias: identity.alias,
    };

    Ok(serde_json::to_value(response).unwrap())
}

fn handle_identity_init(ctx: &DaemonContext) -> Result<Value, JsonRpcError> {
    // Check if already exists
    if ctx.identity_store.exists() {
        return Err(JsonRpcError::identity_exists());
    }

    // Initialize
    let (_keypair, identity) = ctx
        .identity_store
        .init()
        .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

    // Trust self
    ctx.trust_store
        .trust_self(&identity.public_key)
        .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

    // Log
    let _ = ctx.log_writer.info("identity", "Identity initialized");

    let response = IdentityGetResponse {
        public_key: identity.public_key,
        created_at: identity.created_at.to_rfc3339(),
        alias: identity.alias,
    };

    Ok(serde_json::to_value(response).unwrap())
}

fn handle_identity_sign(ctx: &DaemonContext, params: &Value) -> Result<Value, JsonRpcError> {
    let request: IdentitySignRequest = serde_json::from_value(params.clone())
        .map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let keypair = ctx
        .identity_store
        .load_keypair()
        .map_err(|_| JsonRpcError::identity_not_initialized())?;

    let payload = BASE64
        .decode(&request.payload)
        .map_err(|_| JsonRpcError::invalid_params("Invalid base64 payload"))?;

    let signature = keypair.sign_base64(&payload);

    let response = IdentitySignResponse { signature };
    Ok(serde_json::to_value(response).unwrap())
}

fn handle_identity_set_alias(ctx: &DaemonContext, params: &Value) -> Result<Value, JsonRpcError> {
    let request: IdentitySetAliasRequest = serde_json::from_value(params.clone())
        .map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    ctx.identity_store
        .set_alias(&request.alias)
        .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

    Ok(serde_json::json!({}))
}

fn handle_identity_export(ctx: &DaemonContext) -> Result<Value, JsonRpcError> {
    let bundle = ctx
        .identity_store
        .export()
        .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

    Ok(serde_json::json!({ "bundle": bundle }))
}

fn handle_identity_import(ctx: &DaemonContext, params: &Value) -> Result<Value, JsonRpcError> {
    let bundle = params
        .get("bundle")
        .and_then(|v| v.as_str())
        .ok_or_else(|| JsonRpcError::invalid_params("Missing 'bundle' parameter"))?;

    let identity = ctx
        .identity_store
        .import(bundle)
        .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

    // Trust self after import
    ctx.trust_store
        .trust_self(&identity.public_key)
        .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

    let response = IdentityGetResponse {
        public_key: identity.public_key,
        created_at: identity.created_at.to_rfc3339(),
        alias: identity.alias,
    };

    Ok(serde_json::to_value(response).unwrap())
}

fn handle_entitlements_can(ctx: &DaemonContext, params: &Value) -> Result<Value, JsonRpcError> {
    let request: EntitlementsCanRequest = serde_json::from_value(params.clone())
        .map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let identity = ctx
        .identity_store
        .load_identity()
        .map_err(|_| JsonRpcError::identity_not_initialized())?;

    let (allowed, reason) = ctx
        .grant_store
        .can(&identity.public_key, &request.feature)
        .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

    let response = EntitlementsCanResponse { allowed, reason };
    Ok(serde_json::to_value(response).unwrap())
}

fn handle_entitlements_list(ctx: &DaemonContext) -> Result<Value, JsonRpcError> {
    let grants = ctx
        .grant_store
        .list()
        .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

    Ok(serde_json::json!({ "grants": grants }))
}

fn handle_entitlements_add(ctx: &DaemonContext, params: &Value) -> Result<Value, JsonRpcError> {
    let request: EntitlementsAddRequest = serde_json::from_value(params.clone())
        .map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let grant: Grant = serde_json::from_value(request.grant)
        .map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let id = ctx
        .grant_store
        .add(grant)
        .map_err(|e| match e {
            crate::entitlements::StoreError::IssuerNotTrusted(issuer) => {
                JsonRpcError::issuer_not_trusted(&issuer)
            }
            _ => JsonRpcError::internal_error(&e.to_string()),
        })?;

    let response = EntitlementsAddResponse { id };
    Ok(serde_json::to_value(response).unwrap())
}

fn handle_entitlements_remove(ctx: &DaemonContext, params: &Value) -> Result<Value, JsonRpcError> {
    let request: EntitlementsRemoveRequest = serde_json::from_value(params.clone())
        .map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    ctx.grant_store
        .remove(&request.id)
        .map_err(|e| match e {
            crate::entitlements::StoreError::NotFound(id) => JsonRpcError::grant_not_found(&id),
            _ => JsonRpcError::internal_error(&e.to_string()),
        })?;

    Ok(serde_json::json!({}))
}

fn handle_config_get(ctx: &DaemonContext, params: &Value) -> Result<Value, JsonRpcError> {
    let request: ConfigGetRequest = serde_json::from_value(params.clone())
        .map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let value = ctx
        .config_store
        .get(&request.namespace, &request.key)
        .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

    let json_value = value.map(toml_to_json);

    let response = ConfigGetResponse { value: json_value };
    Ok(serde_json::to_value(response).unwrap())
}

fn handle_config_set(ctx: &DaemonContext, params: &Value) -> Result<Value, JsonRpcError> {
    let request: ConfigSetRequest = serde_json::from_value(params.clone())
        .map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let toml_value = json_to_toml(request.value)
        .ok_or_else(|| JsonRpcError::invalid_params("Cannot convert value to TOML"))?;

    ctx.config_store
        .set(&request.namespace, &request.key, toml_value)
        .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

    Ok(serde_json::json!({}))
}

fn handle_config_list(ctx: &DaemonContext, params: &Value) -> Result<Value, JsonRpcError> {
    let request: ConfigListRequest = serde_json::from_value(params.clone())
        .map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    let values = ctx
        .config_store
        .list(&request.namespace)
        .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

    let json_values: serde_json::Map<String, Value> = values
        .into_iter()
        .map(|(k, v)| (k, toml_to_json(v)))
        .collect();

    Ok(serde_json::json!({ "entries": json_values }))
}

fn handle_config_reset(ctx: &DaemonContext, params: &Value) -> Result<Value, JsonRpcError> {
    let request: ConfigListRequest = serde_json::from_value(params.clone())
        .map_err(|e| JsonRpcError::invalid_params(&e.to_string()))?;

    ctx.config_store
        .reset(&request.namespace)
        .map_err(|e| JsonRpcError::internal_error(&e.to_string()))?;

    Ok(serde_json::json!({}))
}
