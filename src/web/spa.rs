//! Interface React (dossier `ui/`, compilée dans `ui/dist`) : fichiers statiques
//! et `index.html` pour toutes les adresses qui ne sont ni l'API ni la connexion.
//! Le routage (pages, anciennes adresses `/stats/…`) se fait dans le navigateur.

use crate::context::Context;
use axum::{
    extract::State,
    http::{HeaderValue, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use std::path::{Component, Path, PathBuf};

/// Fichiers dont le nom contient une empreinte (`/assets/index-DU_ummdE.js`) : immuables.
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
/// `index.html` et fichiers sans empreinte (dont `sw.js` et `manifest.webmanifest`,
/// qu'un cache long empêcherait de mettre à jour) : toujours revalidés.
const REVALIDATE: &str = "no-cache";

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase).as_deref() {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff2") => "font/woff2",
        Some("woff") => "font/woff",
        Some("txt") => "text/plain; charset=utf-8",
        Some("webmanifest") => "application/manifest+json",
        _ => "application/octet-stream",
    }
}

/// Chemin demandé, ramené sous le dossier de l'interface (refuse `..` et les chemins absolus).
fn resolve(root: &Path, uri_path: &str) -> Option<PathBuf> {
    let relative = Path::new(uri_path.trim_start_matches('/'));
    if relative.components().any(|c| !matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(root.join(relative))
}

async fn file(path: &Path, cache: &'static str) -> Option<Response> {
    let bytes = tokio::fs::read(path).await.ok()?;
    let mut response = bytes.into_response();
    let headers = response.headers_mut();
    headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type(path)));
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static(cache));
    Some(response)
}

/// `index.html` de l'interface, ou une explication si elle n'a pas été compilée.
pub async fn index(ctx: &Context) -> Response {
    match file(&ctx.config.web.ui_dir.join("index.html"), REVALIDATE).await {
        Some(response) => response,
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
            "Interface web absente : compiler `ui/` (npm ci && npm run build) ou régler WEB_UI_DIR.",
        )
            .into_response(),
    }
}

/// Repli du routeur : fichier de l'interface s'il existe, sinon `index.html`
/// (route de l'application). Un fichier d'`/assets/` introuvable reste un 404 :
/// renvoyer la page à la place d'un script casserait le navigateur sans message clair.
pub async fn fallback(State(ctx): State<Context>, method: Method, uri: Uri) -> Response {
    if method != Method::GET && method != Method::HEAD {
        return StatusCode::METHOD_NOT_ALLOWED.into_response();
    }
    let path = uri.path();
    if path != "/"
        && let Some(target) = resolve(&ctx.config.web.ui_dir, path)
        && target.is_file()
    {
        let cache = if path.starts_with("/assets/") { IMMUTABLE } else { REVALIDATE };
        if let Some(response) = file(&target, cache).await {
            return response;
        }
    }
    if path.starts_with("/assets/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    index(&ctx).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stays_inside_the_ui_folder() {
        let root = Path::new("/srv/ui");
        assert_eq!(resolve(root, "/assets/app.js"), Some(PathBuf::from("/srv/ui/assets/app.js")));
        assert_eq!(resolve(root, "/../etc/passwd"), None);
        assert_eq!(resolve(root, "/assets/../../x"), None);
        assert_eq!(resolve(root, "//etc/passwd"), Some(PathBuf::from("/srv/ui/etc/passwd")));
        assert_eq!(content_type(Path::new("a/b.WOFF2")), "font/woff2");
    }
}
