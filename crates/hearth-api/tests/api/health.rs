use crate::common::{send_json, test_app};
use serde_json::Value;

#[tokio::test]
#[ignore] // requires PostgreSQL
async fn healthz_returns_ok() {
    let (app, _db) = test_app().await;
    let (status, body): (_, Value) = send_json(&app, "GET", "/healthz", None, None).await;
    assert_eq!(status, 200);
    assert_eq!(body["status"], "ok");
}
