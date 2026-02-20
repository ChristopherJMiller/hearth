use axum::Json;
use axum::extract::State;
use hearth_common::api_types::FleetStats;

use crate::AppState;
use crate::auth::UserIdentity;
use crate::error::AppError;
use crate::repo;

pub async fn fleet_stats(
    _user: UserIdentity,
    State(state): State<AppState>,
) -> Result<Json<FleetStats>, AppError> {
    let stats = repo::get_fleet_stats(&state.pool).await?;
    Ok(Json(stats))
}
