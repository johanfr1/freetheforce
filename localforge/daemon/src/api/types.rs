//! JSON-RPC request and response types

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

impl JsonRpcRequest {
    /// Create a new request
    pub fn new(method: &str, params: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::Number(1.into())),
            method: method.to_string(),
            params,
        }
    }
}

/// JSON-RPC 2.0 response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// Create a success response
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    /// Create an error response
    pub fn error(id: Option<Value>, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

/// JSON-RPC 2.0 error
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcError {
    /// Create a new error
    pub fn new(code: i32, message: &str) -> Self {
        Self {
            code,
            message: message.to_string(),
            data: None,
        }
    }

    /// Create an error with additional data
    pub fn with_data(code: i32, message: &str, data: Value) -> Self {
        Self {
            code,
            message: message.to_string(),
            data: Some(data),
        }
    }

    // Standard JSON-RPC errors
    pub fn parse_error() -> Self {
        Self::new(-32700, "Parse error")
    }

    pub fn invalid_request() -> Self {
        Self::new(-32600, "Invalid Request")
    }

    pub fn method_not_found(method: &str) -> Self {
        Self::new(-32601, &format!("Method not found: {}", method))
    }

    pub fn invalid_params(message: &str) -> Self {
        Self::new(-32602, message)
    }

    pub fn internal_error(message: &str) -> Self {
        Self::new(-32603, message)
    }

    // Application-specific errors
    pub fn identity_not_initialized() -> Self {
        Self::with_data(
            -32001,
            "Identity not initialized",
            serde_json::json!({ "hint": "Run 'forge init' first" }),
        )
    }

    pub fn identity_exists() -> Self {
        Self::new(-32002, "Identity already exists")
    }

    pub fn issuer_not_trusted(issuer: &str) -> Self {
        Self::with_data(
            -32003,
            "Issuer not trusted",
            serde_json::json!({ "issuer": issuer }),
        )
    }

    pub fn grant_not_found(id: &str) -> Self {
        Self::new(-32004, &format!("Grant not found: {}", id))
    }

    pub fn namespace_not_found(namespace: &str) -> Self {
        Self::new(-32005, &format!("Namespace not found: {}", namespace))
    }
}

// API-specific request/response types

/// Identity get response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentityGetResponse {
    pub public_key: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
}

/// Identity sign request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentitySignRequest {
    pub payload: String, // base64
}

/// Identity sign response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentitySignResponse {
    pub signature: String, // base64
}

/// Identity set alias request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentitySetAliasRequest {
    pub alias: String,
}

/// Entitlements can request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementsCanRequest {
    pub feature: String,
}

/// Entitlements can response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementsCanResponse {
    pub allowed: bool,
    pub reason: String,
}

/// Entitlements add request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementsAddRequest {
    pub grant: Value,
}

/// Entitlements add response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementsAddResponse {
    pub id: String,
}

/// Entitlements remove request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntitlementsRemoveRequest {
    pub id: String,
}

/// Config get request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigGetRequest {
    pub namespace: String,
    pub key: String,
}

/// Config get response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigGetResponse {
    pub value: Option<Value>,
}

/// Config set request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigSetRequest {
    pub namespace: String,
    pub key: String,
    pub value: Value,
}

/// Config list request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigListRequest {
    pub namespace: String,
}

/// Status response
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusResponse {
    pub version: String,
    pub status: String,
    pub uptime_seconds: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity: Option<String>,
    pub grants_active: usize,
    pub grants_expired: usize,
}
