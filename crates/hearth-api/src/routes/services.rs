use axum::Json;
use axum::extract::State;
use hearth_common::api_types::ServiceInfo;

use crate::AppState;

pub async fn list_services(State(state): State<AppState>) -> Json<Vec<ServiceInfo>> {
    Json(state.services.clone())
}
