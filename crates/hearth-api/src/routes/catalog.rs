use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use hearth_common::api_types::{
    CatalogEntry, CreateCatalogEntryRequest, SoftwareRequest, SoftwareRequestBody,
    UpdateCatalogEntryRequest,
};
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::repo;

pub async fn list_catalog(
    State(state): State<AppState>,
) -> Result<Json<Vec<CatalogEntry>>, AppError> {
    let rows = repo::list_catalog(&state.pool).await?;
    let entries: Vec<CatalogEntry> = rows.into_iter().map(Into::into).collect();
    Ok(Json(entries))
}

pub async fn get_catalog_entry(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<CatalogEntry>, AppError> {
    let row = repo::get_catalog_entry(&state.pool, id).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!("catalog entry {id} not found"))),
    }
}

pub async fn create_catalog_entry(
    State(state): State<AppState>,
    Json(req): Json<CreateCatalogEntryRequest>,
) -> Result<(StatusCode, Json<CatalogEntry>), AppError> {
    let row = repo::create_catalog_entry(&state.pool, &req).await?;
    Ok((StatusCode::CREATED, Json(row.into())))
}

pub async fn update_catalog_entry(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateCatalogEntryRequest>,
) -> Result<Json<CatalogEntry>, AppError> {
    let row = repo::update_catalog_entry(&state.pool, id, &req).await?;
    match row {
        Some(r) => Ok(Json(r.into())),
        None => Err(AppError::NotFound(format!("catalog entry {id} not found"))),
    }
}

pub async fn delete_catalog_entry(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, AppError> {
    let deleted = repo::delete_catalog_entry(&state.pool, id).await?;
    if deleted {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound(format!("catalog entry {id} not found")))
    }
}

pub async fn request_software(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<SoftwareRequestBody>,
) -> Result<(StatusCode, Json<SoftwareRequest>), AppError> {
    let row = repo::create_software_request(
        &state.pool,
        id,
        req.machine_id,
        &req.username,
        req.user_role.as_deref(),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(row.into())))
}
