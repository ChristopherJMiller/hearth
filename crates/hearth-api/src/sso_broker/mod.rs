pub mod client_auth;
pub mod clients;
pub mod config;
pub mod handlers;
pub mod kanidm_upstream;
pub mod keys;
pub mod pkce;
pub mod sessions;
pub mod store;
pub mod tokens;

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
        .route(
            "/authorize",
            axum::routing::get(handlers::authorize::authorize),
        )
        .route(
            "/callback",
            axum::routing::get(handlers::callback::callback),
        )
        .route("/token", axum::routing::post(handlers::token::token))
        .route(
            "/userinfo",
            axum::routing::get(handlers::userinfo::userinfo),
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
