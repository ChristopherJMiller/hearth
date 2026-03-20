//! Headscale REST API client for mesh VPN management.
//!
//! Communicates with the Headscale coordination server to create pre-auth keys
//! for fleet device enrollment and query node information.

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum HeadscaleError {
    #[error("headscale request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("headscale API error ({status}): {body}")]
    Api { status: u16, body: String },
}

/// A Headscale REST API client.
#[derive(Clone, Debug)]
pub struct HeadscaleClient {
    base_url: String,
    api_key: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize)]
struct CreatePreAuthKeyRequest {
    user: String,
    reusable: bool,
    ephemeral: bool,
    expiration: String,
}

#[derive(Debug, Deserialize)]
struct CreatePreAuthKeyResponse {
    #[serde(rename = "preAuthKey")]
    pre_auth_key: PreAuthKey,
}

#[derive(Debug, Deserialize)]
struct PreAuthKey {
    key: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HeadscaleNode {
    pub id: String,
    pub name: String,
    #[serde(rename = "ipAddresses")]
    pub ip_addresses: Vec<String>,
    pub online: bool,
}

#[derive(Debug, Deserialize)]
struct ListNodesResponse {
    nodes: Vec<HeadscaleNode>,
}

impl HeadscaleClient {
    /// Create a new Headscale client from a base URL and API key.
    pub fn new(base_url: &str, api_key: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// Create from environment variables. Returns `None` if `HEADSCALE_URL` is not set.
    pub fn from_env() -> Option<Self> {
        let url = std::env::var("HEADSCALE_URL").ok()?;
        let key = std::env::var("HEADSCALE_API_KEY").unwrap_or_default();
        Some(Self::new(&url, &key))
    }

    /// Returns the base URL of the Headscale server.
    pub fn url(&self) -> &str {
        &self.base_url
    }

    /// Create a pre-auth key for a fleet device to join the mesh.
    pub async fn create_preauth_key(
        &self,
        reusable: bool,
        ephemeral: bool,
        expiry_secs: u64,
    ) -> Result<String, HeadscaleError> {
        let expiration = chrono::Utc::now() + chrono::Duration::seconds(expiry_secs as i64);

        let body = CreatePreAuthKeyRequest {
            user: "hearth".to_string(),
            reusable,
            ephemeral,
            expiration: expiration.to_rfc3339(),
        };

        let resp = self
            .client
            .post(format!("{}/api/v1/preauthkey", self.base_url))
            .bearer_auth(&self.api_key)
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(HeadscaleError::Api {
                status: status.as_u16(),
                body,
            });
        }

        let result: CreatePreAuthKeyResponse = resp.json().await?;
        Ok(result.pre_auth_key.key)
    }

    /// List all nodes registered in Headscale.
    pub async fn list_nodes(&self) -> Result<Vec<HeadscaleNode>, HeadscaleError> {
        let resp = self
            .client
            .get(format!("{}/api/v1/node", self.base_url))
            .bearer_auth(&self.api_key)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(HeadscaleError::Api {
                status: status.as_u16(),
                body,
            });
        }

        let result: ListNodesResponse = resp.json().await?;
        Ok(result.nodes)
    }
}
