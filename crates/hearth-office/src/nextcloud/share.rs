use crate::nextcloud::{NextcloudClient, OcsMeta};
use serde::Deserialize;

#[derive(Debug)]
pub struct ShareLink {
    pub url: String,
    pub token: String,
}

const SHARE_TYPE_PUBLIC_LINK: &str = "3";
const PERMISSIONS_READ_ONLY: &str = "1";

pub fn create_share_link(
    client: &NextcloudClient,
    nc_path: &str,
) -> Result<ShareLink, ShareError> {
    let url = client.ocs_url("/apps/files_sharing/api/v1/shares");

    let resp = client
        .authed_post(&url)
        .header("Content-Type", "application/x-www-form-urlencoded")
        .form(&[
            ("path", nc_path),
            ("shareType", SHARE_TYPE_PUBLIC_LINK),
            ("permissions", PERMISSIONS_READ_ONLY),
        ])
        .send()
        .map_err(ShareError::Request)?;

    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().unwrap_or_default();
        return Err(ShareError::ApiError { status: status.as_u16(), body });
    }

    let ocs_resp: OcsResponse = resp.json().map_err(ShareError::ParseResponse)?;

    match ocs_resp.ocs.meta.statuscode {
        200 => {
            let data = ocs_resp.ocs.data.ok_or(ShareError::MissingData)?;
            Ok(ShareLink { url: data.url, token: data.token })
        }
        code => Err(ShareError::OcsError {
            code,
            message: ocs_resp.ocs.meta.message.unwrap_or_default(),
        }),
    }
}

pub fn copy_to_clipboard(url: &str) -> Result<(), ShareError> {
    let mut clipboard = arboard::Clipboard::new().map_err(ShareError::Clipboard)?;
    clipboard.set_text(url).map_err(ShareError::Clipboard)?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct OcsResponse {
    ocs: OcsEnvelope,
}

#[derive(Debug, Deserialize)]
struct OcsEnvelope {
    meta: OcsMeta,
    data: Option<OcsShareData>,
}

#[derive(Debug, Deserialize)]
struct OcsShareData {
    url: String,
    token: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ShareError {
    #[error("HTTP request failed: {0}")]
    Request(reqwest::Error),

    #[error("API returned HTTP {status}: {body}")]
    ApiError { status: u16, body: String },

    #[error("failed to parse API response: {0}")]
    ParseResponse(reqwest::Error),

    #[error("OCS error {code}: {message}")]
    OcsError { code: u16, message: String },

    #[error("API response missing share data")]
    MissingData,

    #[error("clipboard error: {0}")]
    Clipboard(arboard::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ocs_share_response() {
        let json = r#"{
            "ocs": {
                "meta": { "statuscode": 200, "message": "OK" },
                "data": {
                    "url": "https://cloud.example.com/s/AbCdEfGh12",
                    "token": "AbCdEfGh12",
                    "id": 42,
                    "share_type": 3,
                    "permissions": 1
                }
            }
        }"#;

        let resp: OcsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.ocs.meta.statuscode, 200);
        let data = resp.ocs.data.unwrap();
        assert_eq!(data.url, "https://cloud.example.com/s/AbCdEfGh12");
        assert_eq!(data.token, "AbCdEfGh12");
    }

    #[test]
    fn parse_ocs_error_response() {
        let json = r#"{
            "ocs": {
                "meta": { "statuscode": 404, "message": "Wrong path" },
                "data": null
            }
        }"#;

        let resp: OcsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.ocs.meta.statuscode, 404);
        assert!(resp.ocs.data.is_none());
    }
}
