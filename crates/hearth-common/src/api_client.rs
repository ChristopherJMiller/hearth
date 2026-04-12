//! API client trait and reqwest implementation for communicating with the control plane.

use std::future::Future;

use crate::api_types::*;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Server returned {status}: {body}")]
    Server { status: u16, body: String },
    #[error("Deserialization error: {0}")]
    Deserialize(#[from] serde_json::Error),
}

/// Trait for interacting with the Hearth control plane API.
/// Enables mocking in tests.
///
/// All methods return `Send` futures so they can be used with `tokio::spawn`.
pub trait HearthApiClient: Send + Sync {
    fn get_target_state(
        &self,
        machine_id: Uuid,
    ) -> impl Future<Output = Result<TargetState, ApiError>> + Send;
    fn send_heartbeat(
        &self,
        req: &HeartbeatRequest,
    ) -> impl Future<Output = Result<HeartbeatResponse, ApiError>> + Send;
    fn register_machine(
        &self,
        req: &CreateMachineRequest,
    ) -> impl Future<Output = Result<Machine, ApiError>> + Send;
    fn get_catalog(&self) -> impl Future<Output = Result<Vec<CatalogEntry>, ApiError>> + Send;
    fn request_software(
        &self,
        catalog_entry_id: Uuid,
        machine_id: Uuid,
        username: &str,
    ) -> impl Future<Output = Result<SoftwareRequest, ApiError>> + Send;
    fn claim_install(&self, request_id: Uuid) -> impl Future<Output = Result<(), ApiError>> + Send;
    fn report_install_result(
        &self,
        report: &InstallResultReport,
    ) -> impl Future<Output = Result<(), ApiError>> + Send;
    fn enroll(
        &self,
        req: &EnrollmentRequest,
    ) -> impl Future<Output = Result<EnrollmentResponse, ApiError>> + Send;
    fn get_enrollment_status(
        &self,
        machine_id: Uuid,
    ) -> impl Future<Output = Result<EnrollmentResponse, ApiError>> + Send;
    fn report_user_env(
        &self,
        machine_id: Uuid,
        username: &str,
        role: &str,
        status: UserEnvStatus,
    ) -> impl Future<Output = Result<(), ApiError>> + Send;
    fn report_user_login(
        &self,
        machine_id: Uuid,
        username: &str,
    ) -> impl Future<Output = Result<(), ApiError>> + Send;
    fn report_update_status(
        &self,
        deployment_id: Uuid,
        machine_id: Uuid,
        status: MachineUpdateStatus,
        error_message: Option<&str>,
    ) -> impl Future<Output = Result<(), ApiError>> + Send;

    fn report_action_result(
        &self,
        report: &ActionResultReport,
    ) -> impl Future<Output = Result<(), ApiError>> + Send;

    /// Look up a user's pre-built environment closure from the control plane.
    fn get_user_env_closure(
        &self,
        username: &str,
    ) -> impl Future<Output = Result<UserEnvClosureResponse, ApiError>> + Send;

    /// Request a fresh binary cache token from the API.
    fn get_cache_token(&self) -> impl Future<Output = Result<CacheTokenResponse, ApiError>> + Send;

    /// Update the bearer token at runtime (e.g. after a refresh from heartbeat).
    /// Default implementation is a no-op for test mocks.
    fn update_token(&self, _token: &str) {}
}

/// Production API client using reqwest.
#[derive(Debug, Clone)]
pub struct ReqwestApiClient {
    client: reqwest::Client,
    base_url: String,
    bearer_token: std::sync::Arc<std::sync::RwLock<Option<String>>>,
}

impl ReqwestApiClient {
    pub fn new(base_url: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");
        Self {
            client,
            base_url,
            bearer_token: std::sync::Arc::new(std::sync::RwLock::new(None)),
        }
    }

    pub fn new_with_token(base_url: String, token: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .expect("failed to build HTTP client");
        Self {
            client,
            base_url,
            bearer_token: std::sync::Arc::new(std::sync::RwLock::new(Some(token))),
        }
    }

    /// Update the bearer token at runtime (e.g. after a refresh from heartbeat).
    pub fn set_token(&self, token: String) {
        *self.bearer_token.write().unwrap() = Some(token);
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.trim_end_matches('/'), path)
    }

    fn authed_get(&self, url: String) -> reqwest::RequestBuilder {
        let mut req = self.client.get(url);
        if let Some(token) = self.bearer_token.read().unwrap().as_deref() {
            req = req.bearer_auth(token);
        }
        req
    }

    fn authed_post(&self, url: String) -> reqwest::RequestBuilder {
        let mut req = self.client.post(url);
        if let Some(token) = self.bearer_token.read().unwrap().as_deref() {
            req = req.bearer_auth(token);
        }
        req
    }

    fn authed_put(&self, url: String) -> reqwest::RequestBuilder {
        let mut req = self.client.put(url);
        if let Some(token) = self.bearer_token.read().unwrap().as_deref() {
            req = req.bearer_auth(token);
        }
        req
    }

    async fn check_response(&self, resp: reqwest::Response) -> Result<reqwest::Response, ApiError> {
        if resp.status().is_success() {
            Ok(resp)
        } else {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            Err(ApiError::Server { status, body })
        }
    }
}

impl HearthApiClient for ReqwestApiClient {
    async fn get_target_state(&self, machine_id: Uuid) -> Result<TargetState, ApiError> {
        let resp = self
            .authed_get(self.url(&format!("/api/v1/machines/{machine_id}/target-state")))
            .send()
            .await?;
        let resp = self.check_response(resp).await?;
        Ok(resp.json().await?)
    }

    async fn send_heartbeat(&self, req: &HeartbeatRequest) -> Result<HeartbeatResponse, ApiError> {
        let resp = self
            .authed_post(self.url("/api/v1/heartbeat"))
            .json(req)
            .send()
            .await?;
        let resp = self.check_response(resp).await?;
        Ok(resp.json().await?)
    }

    async fn register_machine(&self, req: &CreateMachineRequest) -> Result<Machine, ApiError> {
        let resp = self
            .authed_post(self.url("/api/v1/machines"))
            .json(req)
            .send()
            .await?;
        let resp = self.check_response(resp).await?;
        Ok(resp.json().await?)
    }

    async fn get_catalog(&self) -> Result<Vec<CatalogEntry>, ApiError> {
        let resp = self.authed_get(self.url("/api/v1/catalog")).send().await?;
        let resp = self.check_response(resp).await?;
        Ok(resp.json().await?)
    }

    async fn request_software(
        &self,
        catalog_entry_id: Uuid,
        machine_id: Uuid,
        username: &str,
    ) -> Result<SoftwareRequest, ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            machine_id: Uuid,
            username: &'a str,
        }
        let resp = self
            .authed_post(self.url(&format!("/api/v1/catalog/{catalog_entry_id}/request")))
            .json(&Body {
                machine_id,
                username,
            })
            .send()
            .await?;
        let resp = self.check_response(resp).await?;
        Ok(resp.json().await?)
    }

    async fn claim_install(&self, request_id: Uuid) -> Result<(), ApiError> {
        let resp = self
            .authed_post(self.url(&format!("/api/v1/requests/{request_id}/claim")))
            .send()
            .await?;
        self.check_response(resp).await?;
        Ok(())
    }

    async fn report_install_result(&self, report: &InstallResultReport) -> Result<(), ApiError> {
        let resp = self
            .authed_post(self.url(&format!("/api/v1/requests/{}/result", report.request_id)))
            .json(report)
            .send()
            .await?;
        self.check_response(resp).await?;
        Ok(())
    }

    async fn enroll(&self, req: &EnrollmentRequest) -> Result<EnrollmentResponse, ApiError> {
        let resp = self
            .authed_post(self.url("/api/v1/enroll"))
            .json(req)
            .send()
            .await?;
        let resp = self.check_response(resp).await?;
        Ok(resp.json().await?)
    }

    async fn get_enrollment_status(
        &self,
        machine_id: Uuid,
    ) -> Result<EnrollmentResponse, ApiError> {
        let resp = self
            .authed_get(self.url(&format!("/api/v1/machines/{machine_id}/enrollment-status")))
            .send()
            .await?;
        let resp = self.check_response(resp).await?;
        Ok(resp.json().await?)
    }

    async fn report_user_env(
        &self,
        machine_id: Uuid,
        username: &str,
        role: &str,
        status: UserEnvStatus,
    ) -> Result<(), ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            role: &'a str,
            status: UserEnvStatus,
        }
        let resp = self
            .authed_put(self.url(&format!(
                "/api/v1/machines/{machine_id}/environments/{username}"
            )))
            .json(&Body { role, status })
            .send()
            .await?;
        self.check_response(resp).await?;
        Ok(())
    }

    async fn report_user_login(&self, machine_id: Uuid, username: &str) -> Result<(), ApiError> {
        let resp = self
            .authed_post(self.url(&format!(
                "/api/v1/machines/{machine_id}/environments/{username}/login"
            )))
            .send()
            .await?;
        self.check_response(resp).await?;
        Ok(())
    }

    async fn report_update_status(
        &self,
        deployment_id: Uuid,
        machine_id: Uuid,
        status: MachineUpdateStatus,
        error_message: Option<&str>,
    ) -> Result<(), ApiError> {
        #[derive(Serialize)]
        struct Body<'a> {
            status: MachineUpdateStatus,
            error_message: Option<&'a str>,
        }
        let resp = self
            .authed_put(self.url(&format!(
                "/api/v1/deployments/{deployment_id}/machines/{machine_id}"
            )))
            .json(&Body {
                status,
                error_message,
            })
            .send()
            .await?;
        self.check_response(resp).await?;
        Ok(())
    }

    async fn report_action_result(&self, report: &ActionResultReport) -> Result<(), ApiError> {
        let resp = self
            .authed_post(self.url(&format!("/api/v1/actions/{}/result", report.action_id)))
            .json(report)
            .send()
            .await?;
        self.check_response(resp).await?;
        Ok(())
    }

    async fn get_user_env_closure(
        &self,
        username: &str,
    ) -> Result<UserEnvClosureResponse, ApiError> {
        let resp = self
            .authed_get(self.url(&format!("/api/v1/users/{username}/env-closure")))
            .send()
            .await?;
        let resp = self.check_response(resp).await?;
        let body = resp.json::<UserEnvClosureResponse>().await?;
        Ok(body)
    }

    async fn get_cache_token(&self) -> Result<CacheTokenResponse, ApiError> {
        let resp = self
            .authed_post(self.url("/api/v1/cache-token"))
            .send()
            .await?;
        let resp = self.check_response(resp).await?;
        Ok(resp.json::<CacheTokenResponse>().await?)
    }

    fn update_token(&self, token: &str) {
        self.set_token(token.to_string());
    }
}
