use crate::nextcloud::{NextcloudClient, OcsMeta};
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct FileComment {
    pub id: String,
    pub author_display_name: String,
    pub message: String,
    pub creation_date_time: DateTime<Utc>,
}

/// Resolve a Nextcloud file path to its internal file ID via WebDAV PROPFIND.
pub fn resolve_file_id(
    client: &NextcloudClient,
    nc_path: &str,
) -> Result<String, CommentsError> {
    let url = client.webdav_file_url(nc_path);

    let propfind_body = r#"<?xml version="1.0" encoding="UTF-8"?>
<d:propfind xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
  <d:prop>
    <oc:fileid/>
  </d:prop>
</d:propfind>"#;

    let resp = client
        .authed_propfind(&url)
        .header("Content-Type", "application/xml")
        .body(propfind_body)
        .send()
        .map_err(CommentsError::Request)?;

    if !resp.status().is_success() && resp.status().as_u16() != 207 {
        return Err(CommentsError::ApiError {
            status: resp.status().as_u16(),
            body: resp.text().unwrap_or_default(),
        });
    }

    let body = resp.text().map_err(CommentsError::Request)?;
    parse_file_id_from_propfind(&body)
}

pub fn get_comments(
    client: &NextcloudClient,
    file_id: &str,
) -> Result<Vec<FileComment>, CommentsError> {
    let url = client.ocs_url(&format!("/apps/dav/api/v1/comments/files/{file_id}"));

    let resp = client
        .authed_get(&url)
        .header("Accept", "application/json")
        .send()
        .map_err(CommentsError::Request)?;

    if !resp.status().is_success() {
        return Err(CommentsError::ApiError {
            status: resp.status().as_u16(),
            body: resp.text().unwrap_or_default(),
        });
    }

    let ocs_resp: OcsCommentsResponse = resp.json().map_err(CommentsError::ParseResponse)?;

    if ocs_resp.ocs.meta.statuscode != 200 {
        return Err(CommentsError::OcsError {
            code: ocs_resp.ocs.meta.statuscode,
            message: ocs_resp.ocs.meta.message.unwrap_or_default(),
        });
    }

    Ok(ocs_resp.ocs.data.unwrap_or_default())
}

pub fn post_comment(
    client: &NextcloudClient,
    file_id: &str,
    message: &str,
) -> Result<(), CommentsError> {
    let url = client.ocs_url(&format!("/apps/dav/api/v1/comments/files/{file_id}"));

    let resp = client
        .authed_post(&url)
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "message": message }))
        .send()
        .map_err(CommentsError::Request)?;

    if !resp.status().is_success() {
        return Err(CommentsError::ApiError {
            status: resp.status().as_u16(),
            body: resp.text().unwrap_or_default(),
        });
    }

    Ok(())
}

fn parse_file_id_from_propfind(xml: &str) -> Result<String, CommentsError> {
    let start_tag = "<oc:fileid>";
    let end_tag = "</oc:fileid>";

    let start = xml.find(start_tag).ok_or(CommentsError::MissingFileId)?;
    let value_start = start + start_tag.len();
    let end = xml[value_start..].find(end_tag).ok_or(CommentsError::MissingFileId)?;

    let file_id = xml[value_start..value_start + end].trim().to_string();
    if file_id.is_empty() {
        return Err(CommentsError::MissingFileId);
    }

    Ok(file_id)
}

#[derive(Debug, Deserialize)]
struct OcsCommentsResponse {
    ocs: OcsCommentsEnvelope,
}

#[derive(Debug, Deserialize)]
struct OcsCommentsEnvelope {
    meta: OcsMeta,
    data: Option<Vec<FileComment>>,
}

#[derive(Debug, thiserror::Error)]
pub enum CommentsError {
    #[error("HTTP request failed: {0}")]
    Request(reqwest::Error),

    #[error("API returned HTTP {status}: {body}")]
    ApiError { status: u16, body: String },

    #[error("failed to parse API response: {0}")]
    ParseResponse(reqwest::Error),

    #[error("OCS error {code}: {message}")]
    OcsError { code: u16, message: String },

    #[error("could not resolve file ID from PROPFIND response")]
    MissingFileId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fileid_from_propfind() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:" xmlns:oc="http://owncloud.org/ns">
  <d:response>
    <d:href>/remote.php/dav/files/alice/Documents/report.odt</d:href>
    <d:propstat>
      <d:prop>
        <oc:fileid>98765</oc:fileid>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

        let id = parse_file_id_from_propfind(xml).unwrap();
        assert_eq!(id, "98765");
    }

    #[test]
    fn parse_fileid_missing_returns_error() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response><d:propstat><d:prop></d:prop></d:propstat></d:response>
</d:multistatus>"#;

        assert!(parse_file_id_from_propfind(xml).is_err());
    }

    #[test]
    fn parse_comments_response() {
        let json = r#"{
            "ocs": {
                "meta": { "statuscode": 200, "message": "OK" },
                "data": [
                    {
                        "id": "1",
                        "author_display_name": "Alice",
                        "message": "Looks good!",
                        "creation_date_time": "2026-04-10T14:30:00Z"
                    },
                    {
                        "id": "2",
                        "author_display_name": "Bob",
                        "message": "Fixed the typo in section 3.",
                        "creation_date_time": "2026-04-11T09:15:00Z"
                    }
                ]
            }
        }"#;

        let resp: OcsCommentsResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.ocs.meta.statuscode, 200);
        let comments = resp.ocs.data.unwrap();
        assert_eq!(comments.len(), 2);
        assert_eq!(comments[0].author_display_name, "Alice");
        assert_eq!(comments[1].message, "Fixed the typo in section 3.");
    }
}
