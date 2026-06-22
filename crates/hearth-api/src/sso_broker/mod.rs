pub mod config;

use crate::AppState;
use axum::Router;

pub fn routes() -> Router<AppState> {
    Router::new()
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
