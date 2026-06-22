pub mod config;
pub mod handlers;
pub mod keys;

use crate::AppState;
use axum::Router;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/.well-known/openid-configuration",
            axum::routing::get(handlers::discovery::discovery),
        )
        .route(
            "/.well-known/jwks.json",
            axum::routing::get(handlers::jwks::jwks),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn module_compiles_and_router_builds() {
        // This test exists so the module compiles before we add real routes.
        let _: Router = routes().with_state(crate::test_helpers::test_app_state());
    }
}
