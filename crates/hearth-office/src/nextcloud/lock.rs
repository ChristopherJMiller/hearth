use crate::nextcloud::NextcloudClient;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockStatus {
    Unlocked,
    Locked {
        owner: String,
        timeout: Option<String>,
    },
}

pub fn check_lock_status(
    client: &NextcloudClient,
    nc_path: &str,
) -> Result<LockStatus, LockError> {
    let url = client.webdav_file_url(nc_path);

    let propfind_body = r#"<?xml version="1.0" encoding="UTF-8"?>
<d:propfind xmlns:d="DAV:">
  <d:prop>
    <d:lockdiscovery/>
  </d:prop>
</d:propfind>"#;

    let resp = client
        .authed_propfind(&url)
        .header("Content-Type", "application/xml")
        .body(propfind_body)
        .send()
        .map_err(LockError::Request)?;

    let status = resp.status().as_u16();
    if status != 207 && !resp.status().is_success() {
        return Err(LockError::ApiError {
            status,
            body: resp.text().unwrap_or_default(),
        });
    }

    let body = resp.text().map_err(LockError::Request)?;
    Ok(parse_lock_status(&body))
}

fn parse_lock_status(xml: &str) -> LockStatus {
    if !xml.contains("<d:activelock>") && !xml.contains("<D:activelock>") {
        return LockStatus::Unlocked;
    }

    let owner = extract_tag_content(xml, "owner")
        .map(|s| {
            // Owner might be wrapped in <d:href>
            extract_tag_content(&s, "href").unwrap_or(s)
        })
        .unwrap_or_else(|| "Unknown".into());

    let timeout = extract_tag_content(xml, "timeout");

    LockStatus::Locked { owner, timeout }
}

/// Extract content of an XML tag, trying bare name and d:/D: namespace prefixes.
fn extract_tag_content(xml: &str, tag: &str) -> Option<String> {
    let patterns = [
        (format!("<{tag}>"), format!("</{tag}>")),
        (format!("<d:{tag}>"), format!("</d:{tag}>")),
        (format!("<D:{tag}>"), format!("</D:{tag}>")),
    ];

    for (open, close) in &patterns {
        if let Some(start) = xml.find(open.as_str()) {
            let value_start = start + open.len();
            if let Some(end) = xml[value_start..].find(close.as_str()) {
                let content = xml[value_start..value_start + end].trim().to_string();
                if !content.is_empty() {
                    return Some(content);
                }
            }
        }
    }
    None
}

#[derive(Debug, thiserror::Error)]
pub enum LockError {
    #[error("HTTP request failed: {0}")]
    Request(reqwest::Error),

    #[error("API returned HTTP {status}: {body}")]
    ApiError { status: u16, body: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unlocked_file() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:propstat>
      <d:prop>
        <d:lockdiscovery/>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

        assert_eq!(parse_lock_status(xml), LockStatus::Unlocked);
    }

    #[test]
    fn locked_file_with_owner() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:propstat>
      <d:prop>
        <d:lockdiscovery>
          <d:activelock>
            <d:locktype><d:write/></d:locktype>
            <d:lockscope><d:exclusive/></d:lockscope>
            <d:owner>bob@hearth.local</d:owner>
            <d:timeout>Second-3600</d:timeout>
            <d:locktoken>
              <d:href>opaquelocktoken:abc123</d:href>
            </d:locktoken>
          </d:activelock>
        </d:lockdiscovery>
      </d:prop>
      <d:status>HTTP/1.1 200 OK</d:status>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

        match parse_lock_status(xml) {
            LockStatus::Locked { owner, timeout } => {
                assert_eq!(owner, "bob@hearth.local");
                assert_eq!(timeout, Some("Second-3600".into()));
            }
            _ => panic!("expected Locked status"),
        }
    }

    #[test]
    fn locked_file_with_href_owner() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response>
    <d:propstat>
      <d:prop>
        <d:lockdiscovery>
          <d:activelock>
            <d:owner><d:href>alice</d:href></d:owner>
          </d:activelock>
        </d:lockdiscovery>
      </d:prop>
    </d:propstat>
  </d:response>
</d:multistatus>"#;

        match parse_lock_status(xml) {
            LockStatus::Locked { owner, .. } => assert_eq!(owner, "alice"),
            _ => panic!("expected Locked status"),
        }
    }

    #[test]
    fn no_lockdiscovery_means_unlocked() {
        let xml = r#"<?xml version="1.0"?>
<d:multistatus xmlns:d="DAV:">
  <d:response><d:propstat><d:prop></d:prop></d:propstat></d:response>
</d:multistatus>"#;

        assert_eq!(parse_lock_status(xml), LockStatus::Unlocked);
    }
}
