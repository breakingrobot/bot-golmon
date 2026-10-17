//! Connexion Discord (OAuth2, scope `identify`) et session dans un cookie signé.
//!
//! Rien n'est stocké côté serveur : le cookie contient l'identifiant Discord,
//! le nom affiché et l'expiration, signés en HMAC-SHA256 avec une clé dérivée
//! du token du bot (stable entre deux redémarrages, jamais exposée). Les droits
//! sont recalculés à chaque requête depuis le cache : retirer un rôle suffit à
//! couper l'accès.

use crate::context::{Context, Ctx};
use axum::{
    extract::{FromRequestParts, Query, State},
    http::{HeaderMap, HeaderValue, header, request::Parts},
    response::{IntoResponse, Redirect, Response},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as B64};
use ring::{hmac, rand::SecureRandom as _};
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use twilight_model::id::{Id, marker::UserMarker};

const SESSION_COOKIE: &str = "session";
const STATE_COOKIE: &str = "oauth_state";
const SESSION_DAYS: i64 = 7;

/// Utilisateur connecté.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Viewer {
    pub id: Id<UserMarker>,
    pub name: String,
    /// Expiration (secondes Unix).
    exp: i64,
}

/// Session éventuelle de la requête (jamais d'échec : `None` si absente ou invalide).
pub struct MaybeViewer(pub Option<Viewer>);

impl FromRequestParts<Context> for MaybeViewer {
    type Rejection = Infallible;

    async fn from_request_parts(parts: &mut Parts, ctx: &Context) -> Result<Self, Self::Rejection> {
        let viewer = cookie(&parts.headers, SESSION_COOKIE).and_then(|value| verify_session(ctx, value));
        Ok(Self(viewer))
    }
}

/// La connexion est possible quand le secret OAuth2 et l'adresse publique sont connus.
pub fn enabled(ctx: &Ctx) -> bool {
    ctx.config.web.client_secret.is_some() && public_url(ctx).is_some()
}

pub fn public_url(ctx: &Ctx) -> Option<&str> {
    ctx.config.web.public_url.as_deref().or_else(|| ctx.modmail.as_ref().and_then(|m| m.config.url.as_deref()))
}

fn key(ctx: &Ctx) -> hmac::Key {
    // Dérivation : la clé de session n'est jamais le token lui-même.
    let derived = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, ctx.config.token.as_bytes()), b"web-session-v1");
    hmac::Key::new(hmac::HMAC_SHA256, derived.as_ref())
}

fn sign(ctx: &Ctx, payload: &[u8]) -> String {
    format!("{}.{}", B64.encode(payload), B64.encode(hmac::sign(&key(ctx), payload)))
}

fn verify(ctx: &Ctx, value: &str) -> Option<Vec<u8>> {
    let (payload, signature) = value.split_once('.')?;
    let payload = B64.decode(payload).ok()?;
    let signature = B64.decode(signature).ok()?;
    hmac::verify(&key(ctx), &payload, &signature).ok()?;
    Some(payload)
}

fn verify_session(ctx: &Ctx, value: &str) -> Option<Viewer> {
    let viewer: Viewer = serde_json::from_slice(&verify(ctx, value)?).ok()?;
    (viewer.exp > chrono::Utc::now().timestamp()).then_some(viewer)
}

fn cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(key, _)| *key == name)
        .map(|(_, value)| value)
}

fn set_cookie(ctx: &Ctx, name: &str, value: &str, max_age: i64) -> HeaderValue {
    let secure = if public_url(ctx).is_some_and(|u| u.starts_with("https://")) { "; Secure" } else { "" };
    HeaderValue::from_str(&format!("{name}={value}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Lax{secure}"))
        .expect("cookie ASCII")
}

/// Chemin de retour accepté après connexion : local uniquement.
fn safe_next(next: Option<&str>) -> String {
    match next {
        Some(path) if path.starts_with('/') && !path.starts_with("//") && !path.contains('\\') => path.to_owned(),
        _ => "/".to_owned(),
    }
}

fn redirect_uri(ctx: &Ctx) -> Option<String> {
    public_url(ctx).map(|url| format!("{url}/auth/callback"))
}

/// Lien de connexion qui ramène sur `next`.
pub fn login_href(next: &str) -> String {
    let query: String = url_encode(next);
    format!("/login?next={query}")
}

pub(crate) fn url_encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

#[derive(Deserialize)]
pub struct LoginQuery {
    next: Option<String>,
}

pub async fn login(State(ctx): State<Context>, Query(query): Query<LoginQuery>) -> Response {
    let (true, Some(redirect_uri)) = (enabled(&ctx), redirect_uri(&ctx)) else {
        return super::pages::message_page(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "Connexion indisponible",
            "La connexion Discord n'est pas configurée (DISCORD_CLIENT_SECRET et WEB_URL).",
        );
    };
    let mut nonce = [0u8; 24];
    if ring::rand::SystemRandom::new().fill(&mut nonce).is_err() {
        return axum::http::StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
    let nonce = B64.encode(nonce);
    let next = safe_next(query.next.as_deref());
    // Le nonce et la page de retour voyagent dans un cookie signé, le nonce seul dans `state`.
    let state_cookie = sign(&ctx, format!("{nonce}\n{next}").as_bytes());

    let url = format!(
        "https://discord.com/oauth2/authorize?response_type=code&client_id={}&scope=identify&redirect_uri={}&state={}",
        ctx.application_id,
        url_encode(&redirect_uri),
        url_encode(&nonce),
    );
    let mut response = Redirect::to(&url).into_response();
    response.headers_mut().append(header::SET_COOKIE, set_cookie(&ctx, STATE_COOKIE, &state_cookie, 600));
    response
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct DiscordUser {
    id: Id<UserMarker>,
    username: String,
    global_name: Option<String>,
}

pub async fn callback(State(ctx): State<Context>, headers: HeaderMap, Query(query): Query<CallbackQuery>) -> Response {
    let refused = || {
        super::pages::message_page(
            axum::http::StatusCode::BAD_REQUEST,
            "Connexion refusée",
            "La connexion a échoué ou a expiré. Réessaie depuis la page d'accueil.",
        )
    };
    let Some(stored) = cookie(&headers, STATE_COOKIE).and_then(|v| verify(&ctx, v)) else {
        return refused();
    };
    let stored = String::from_utf8_lossy(&stored).into_owned();
    let Some((nonce, next)) = stored.split_once('\n') else {
        return refused();
    };
    let (Some(code), Some(state)) = (query.code, query.state) else {
        return refused();
    };
    if state != nonce {
        return refused();
    }

    let user = match exchange(&ctx, &code).await {
        Ok(user) => user,
        Err(error) => {
            tracing::warn!(?error, "connexion Discord échouée");
            return refused();
        }
    };
    let viewer = Viewer {
        id: user.id,
        name: user.global_name.unwrap_or(user.username),
        exp: chrono::Utc::now().timestamp() + SESSION_DAYS * 86_400,
    };
    tracing::info!(user = %viewer.id, "connexion au site");
    let session = sign(&ctx, &serde_json::to_vec(&viewer).expect("session sérialisable"));

    let mut response = Redirect::to(&safe_next(Some(next))).into_response();
    let headers = response.headers_mut();
    headers.append(header::SET_COOKIE, set_cookie(&ctx, SESSION_COOKIE, &session, SESSION_DAYS * 86_400));
    headers.append(header::SET_COOKIE, set_cookie(&ctx, STATE_COOKIE, "", 0));
    response
}

async fn exchange(ctx: &Ctx, code: &str) -> anyhow::Result<DiscordUser> {
    let client = reqwest::Client::new();
    let secret = ctx.config.web.client_secret.as_deref().unwrap_or_default();
    let redirect_uri = redirect_uri(ctx).unwrap_or_default();
    let token: TokenResponse = client
        .post("https://discord.com/api/v10/oauth2/token")
        .basic_auth(ctx.application_id.to_string(), Some(secret))
        .form(&[("grant_type", "authorization_code"), ("code", code), ("redirect_uri", &redirect_uri)])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok(client
        .get("https://discord.com/api/v10/users/@me")
        .bearer_auth(token.access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

pub async fn logout(State(ctx): State<Context>) -> Response {
    let mut response = Redirect::to("/").into_response();
    response.headers_mut().append(header::SET_COOKIE, set_cookie(&ctx, SESSION_COOKIE, "", 0));
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_local_redirects() {
        assert_eq!(safe_next(Some("/logs/abc?simple=1")), "/logs/abc?simple=1");
        assert_eq!(safe_next(Some("//evil.example")), "/");
        assert_eq!(safe_next(Some("https://evil.example")), "/");
        assert_eq!(safe_next(Some("/\\evil.example")), "/");
        assert_eq!(safe_next(None), "/");
    }

    #[test]
    fn encodes_query_values() {
        assert_eq!(url_encode("/logs?q=a b&x"), "/logs%3Fq%3Da%20b%26x");
        assert_eq!(login_href("/stats"), "/login?next=/stats");
    }
}
