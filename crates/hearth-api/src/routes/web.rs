use axum::response::Html;

/// Fallback handler for the catalog SPA.
///
/// In production, the Vite-built `index.html` is served from the dist directory
/// via `ServeDir`. This handler is the fallback for client-side routes — any
/// `/catalog/*` path that doesn't match a static file gets the SPA shell.
///
/// During development, run `pnpm dev` in `web/apps/catalog/` which proxies
/// API requests to the Axum server on :3000.
pub async fn catalog_spa_fallback() -> Html<String> {
    // Try to read the built index.html from the dist directory.
    // If the file doesn't exist (hasn't been built), return a helpful message.
    let dist_path =
        std::env::var("HEARTH_WEB_DIST").unwrap_or_else(|_| "web/apps/catalog/dist".to_string());

    let index_path = std::path::Path::new(&dist_path).join("index.html");

    match std::fs::read_to_string(&index_path) {
        Ok(html) => Html(html),
        Err(_) => Html(format!(
            r#"<!DOCTYPE html>
<html><head><title>Hearth Software Center</title></head>
<body style="background:#1a1a2e;color:#eaeaea;font-family:sans-serif;display:flex;align-items:center;justify-content:center;min-height:100vh">
<div style="text-align:center">
<h1>Hearth Software Center</h1>
<p style="color:#a0a0b0;margin-top:16px">Frontend not built yet.</p>
<p style="color:#a0a0b0;margin-top:8px">Run <code style="color:#e94560">cd web &amp;&amp; pnpm install &amp;&amp; pnpm build</code></p>
<p style="color:#a0a0b0;margin-top:8px">Or for development: <code style="color:#e94560">cd web &amp;&amp; pnpm dev</code> (runs on :5173)</p>
<p style="color:#a0a0b0;margin-top:8px">Looking for dist at: <code style="color:#e94560">{}</code></p>
</div></body></html>"#,
            index_path.display()
        )),
    }
}

/// Fallback handler for the console SPA.
///
/// Serves the admin console web application. Works the same way as the catalog
/// SPA fallback — any `/console/*` path that doesn't match a static file gets
/// the SPA shell so that client-side routing works.
pub async fn console_spa_fallback() -> Html<String> {
    let dist_path = std::env::var("HEARTH_CONSOLE_DIST")
        .unwrap_or_else(|_| "web/apps/console/dist".to_string());

    let index_path = std::path::Path::new(&dist_path).join("index.html");

    match std::fs::read_to_string(&index_path) {
        Ok(html) => Html(html),
        Err(_) => Html(format!(
            r#"<!DOCTYPE html>
<html><head><title>Hearth Console</title></head>
<body style="background:#141726;color:#eaeaea;font-family:sans-serif;display:flex;align-items:center;justify-content:center;min-height:100vh">
<div style="text-align:center">
<h1>Hearth Console</h1>
<p style="color:#a0a0b0;margin-top:16px">Console frontend not built yet.</p>
<p style="color:#a0a0b0;margin-top:8px">Run <code style="color:#e94560">cd web &amp;&amp; pnpm install &amp;&amp; pnpm build</code></p>
<p style="color:#a0a0b0;margin-top:8px">Or for development: <code style="color:#e94560">cd web/apps/console &amp;&amp; pnpm dev</code></p>
<p style="color:#a0a0b0;margin-top:8px">Looking for dist at: <code style="color:#e94560">{}</code></p>
</div></body></html>"#,
            index_path.display()
        )),
    }
}
