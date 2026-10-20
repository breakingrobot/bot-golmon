//! Tests de l'API : routeur complet sur une base temporaire, sans Discord.

use super::*;
use crate::{
    config::{Config, WebConfig},
    context::Ctx,
    modules::audit,
    modules::permissions::{Grants, Holder, Level, Roles},
    web::GuildAccess,
};
use axum::body::Body;
use std::{collections::HashMap, sync::Arc};
use tower::ServiceExt as _;

struct TestApp {
    ctx: Context,
    dir: std::path::PathBuf,
}

impl TestApp {
    async fn new(login: bool) -> Self {
        let dir = std::env::temp_dir().join(format!("api-test-{}", uuid::Uuid::new_v4()));
        let db = crate::db::open_main(&dir.join("bot.db")).await.unwrap();
        let web = WebConfig {
            port: None,
            public_url: login.then(|| "https://bot.example".to_owned()),
            client_secret: login.then(|| "secret".to_owned()),
            ui_dir: dir.join("ui"),
        };
        let config = Config {
            token: "token-de-test".into(),
            guild_whitelist: Vec::new(),
            database_path: dir.join("bot.db"),
            deploy_commands: false,
            web,
        };
        let ctx = Arc::new(Ctx {
            http: twilight_http::Client::new(config.token.clone()),
            config,
            cache: twilight_cache_inmemory::DefaultInMemoryCache::new(),
            db,
            application_id: Id::new(1),
            bot_user_id: Id::new(2),
            bot_username: "bot".into(),
            vehicles: crate::modules::vehicles::api::VehicleApi::new(),
            modmail: None,
            message_content: false,
            shard_senders: std::sync::OnceLock::new(),
            events: crate::events::EventBus::default(),
            health: crate::health::Health::default(),
        });
        Self { ctx, dir }
    }

    async fn send(&self, request: axum::http::Request<Body>) -> (StatusCode, HeaderMap, serde_json::Value) {
        let response = crate::web::app(self.ctx.clone()).oneshot(request).await.unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, headers, value)
    }

    fn cookie(&self) -> String {
        auth::test_session_cookie(&self.ctx, Id::new(42), "Elodie")
    }
}

impl Drop for TestApp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn get(uri: &str) -> axum::http::Request<Body> {
    axum::http::Request::get(uri).body(Body::empty()).unwrap()
}

fn keys(value: &serde_json::Value) -> Vec<String> {
    let mut keys: Vec<String> = value.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    keys
}

#[tokio::test]
async fn me_works_without_login() {
    let app = TestApp::new(false).await;
    let (status, headers, body) = app.send(get("/api/me")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers[header::CONTENT_TYPE].to_str().unwrap().starts_with("application/json"));
    assert_eq!(keys(&body), ["guilds", "loginEnabled", "loginUrl", "tickets", "user"]);
    assert_eq!(body["user"], serde_json::Value::Null);
    assert_eq!(body["loginEnabled"], false);
    assert_eq!(body["loginUrl"], "/login?next=/");

    // Sans connexion configurée : le reste est refusé (401), sans lien de connexion.
    let (status, _, body) = app.send(get("/api/guilds/123/stats")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body["error"].is_string() && body.get("loginUrl").is_none());
}

#[tokio::test]
async fn anonymous_and_logged_in_sessions() {
    let app = TestApp::new(true).await;
    let (status, _, body) = app.send(get("/api/me")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!((body["user"].is_null(), &body["loginEnabled"]), (true, &serde_json::Value::Bool(true)));

    let (status, _, body) = app.send(get("/api/guilds/123/cotisations")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["loginUrl"], "/login?next=/");

    let request = axum::http::Request::get("/api/me").header(header::COOKIE, app.cookie()).body(Body::empty()).unwrap();
    let (_, _, body) = app.send(request).await;
    assert_eq!(keys(&body["user"]), ["avatarUrl", "id", "name"]);
    assert_eq!(body["user"]["id"], "42");
    assert_eq!(body["guilds"], serde_json::json!([]));

    // Connecté, mais pas membre du serveur : 403.
    let request = axum::http::Request::get("/api/guilds/123/stats")
        .header(header::COOKIE, app.cookie())
        .body(Body::empty())
        .unwrap();
    let (status, _, body) = app.send(request).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body["error"].as_str().unwrap().contains("accès"));
}

#[tokio::test]
async fn unknown_api_routes_are_json() {
    let app = TestApp::new(false).await;
    for uri in ["/api/nope", "/api/guilds/1/nope", "/api"] {
        let (status, _, body) = app.send(get(uri)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        assert!(body["error"].is_string(), "{uri}");
    }
    // Spécification publique, même avec la connexion configurée.
    let app_login = TestApp::new(true).await;
    let (status, _, body) = app_login.send(get("/api/openapi.json")).await;
    assert_eq!((status, &body["openapi"]), (StatusCode::OK, &serde_json::json!("3.1.0")));
    // Réglages branchés : sans connexion, refus en JSON (plus de 501).
    let (status, _, body) = app.send(get("/api/guilds/1/config")).await;
    assert!(status.is_client_error() && body["error"].is_string(), "{status}");
}

#[tokio::test]
async fn writes_need_csrf_headers() {
    let app = TestApp::new(true).await;
    let post = |content_type: Option<&str>, fetch: bool| {
        let mut request = axum::http::Request::post("/api/guilds/123/contacts").header(header::COOKIE, app.cookie());
        if let Some(content_type) = content_type {
            request = request.header(header::CONTENT_TYPE, content_type);
        }
        if fetch {
            request = request.header("x-requested-with", "fetch");
        }
        request.body(Body::from(r#"{"name":"x","kind":"GROUPE","relation":"ALLIE"}"#)).unwrap()
    };
    for (content_type, fetch) in [(None, true), (Some("application/json"), false), (Some("text/plain"), true)] {
        let (status, _, body) = app.send(post(content_type, fetch)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert!(body["error"].as_str().unwrap().contains("X-Requested-With"));
    }
    // En-têtes présents : la requête passe la protection (puis 403 : pas membre du serveur).
    let (status, _, body) = app.send(post(Some("application/json; charset=utf-8"), true)).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(body["error"].as_str().unwrap().contains("serveur"));

    assert!(csrf_allowed(&Method::GET, &HeaderMap::new()));
    assert!(!csrf_allowed(&Method::DELETE, &HeaderMap::new()));
}

fn access(levels: &[(Perm, Level)], roles: &[&str], manage_guild: bool) -> Access {
    let grants = Grants::from_parts(
        Roles { member: Some("1".into()), grade: Some("2".into()), lead: Some("3".into()) },
        levels.iter().map(|(p, l)| (p.key().to_owned(), *l)).collect::<HashMap<_, _>>(),
        Holder { roles: roles.iter().map(|r| (*r).to_owned()).collect(), manage_guild, manage_messages: false },
    );
    Access { tickets: false, guilds: vec![GuildAccess::new(Id::new(10), "Serveur".into(), grants, manage_guild)] }
}

#[test]
fn guild_permissions_are_checked() {
    let guild = Id::new(10);
    let member = access(&[], &["1"], false);
    assert!(check_guild(&member, guild, Some(Perm::ContactVoir)).is_ok());
    assert!(check_guild(&member, guild, Some(Perm::ContactAjouter)).is_ok());
    let refused = check_guild(&member, guild, Some(Perm::CoffreVoir)).unwrap_err();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert!(refused.message().contains("gradés"));
    assert_eq!(check_guild(&member, Id::new(11), None).unwrap_err().status(), StatusCode::FORBIDDEN);
    assert_eq!(check_admin(&member, guild).unwrap_err().status(), StatusCode::FORBIDDEN);

    // Niveau abaissé par le serveur.
    let opened = access(&[(Perm::CoffreVoir, Level::Membre)], &["1"], false);
    assert!(check_guild(&opened, guild, Some(Perm::CoffreVoir)).is_ok());

    let admin = access(&[], &[], true);
    assert!(check_admin(&admin, guild).is_ok());
    assert!(admin.guilds[0].perms().contains(&Perm::Journal));
    assert!(!admin.guilds[0].perms().contains(&Perm::RecrueAvis), "action stricte : rôles seulement");
}

#[test]
fn me_shape_matches_the_contract() {
    let ctx_free = access(&[], &["2"], true);
    let guild = &ctx_free.guilds[0];
    let value = serde_json::to_value(me::GuildAccess {
        id: guild.id.to_string(),
        name: guild.name.clone(),
        icon_url: None,
        is_admin: guild.is_admin(),
        in_group: guild.in_group(),
        perms: guild.perms().into_iter().map(|p| p.key()).collect(),
    })
    .unwrap();
    assert_eq!(keys(&value), ["iconUrl", "id", "inGroup", "isAdmin", "name", "perms"]);
    assert_eq!(value["inGroup"], true, "rôle gradé");
    assert!(value["perms"].as_array().unwrap().contains(&serde_json::json!("coffre.voir")));

    let page = serde_json::to_value(Page { items: vec![1], total: 1, page: 1, page_size: 50 }).unwrap();
    assert_eq!(keys(&page), ["items", "page", "pageSize", "total"]);
    let error = serde_json::to_value(ErrorBody { error: "x".into(), login_url: Some("/login".into()) }).unwrap();
    assert_eq!(keys(&error), ["error", "loginUrl"]);
}

#[tokio::test]
async fn cotisation_weekly_series_include_empty_weeks() {
    let app = TestApp::new(false).await;
    let db = &app.ctx.db;
    let day = |raw: &str| chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d").unwrap();
    for (user, propre, sale, date) in
        [("a", 100, 0, "2026-09-28"), ("b", 0, 50, "2026-10-04"), ("a", 10, 5, "2026-10-12")]
    {
        crate::modules::cotisations::service::add(
            db,
            "g",
            crate::modules::cotisations::service::NewCotisation {
                user_id: user.into(),
                propre,
                sale,
                description: "test".into(),
                date: crate::util::time::paris_day_start(day(date)),
                created_by_id: user.into(),
            },
        )
        .await
        .unwrap();
    }
    let weeks = series::weeks(day("2026-09-28"), day("2026-10-18"));
    let all = cotisations::weekly(db, "g", None, None, &weeks).await.unwrap();
    let summary: Vec<(&str, i64, i64)> = all.iter().map(|w| (w.week_start.as_str(), w.propre, w.sale)).collect();
    assert_eq!(summary, [("2026-09-28", 100, 50), ("2026-10-05", 0, 0), ("2026-10-12", 10, 5)]);
    let only_a = cotisations::weekly(db, "g", Some("a"), None, &weeks).await.unwrap();
    assert_eq!(only_a[0].sale, 0);
    let value = serde_json::to_value(&all[0]).unwrap();
    assert_eq!(keys(&value), ["propre", "sale", "weekStart"]);
}

#[tokio::test]
async fn member_profile_gathers_modules() {
    let app = TestApp::new(false).await;
    let db = &app.ctx.db;
    sqlx::query(
        r#"INSERT INTO "PhoneNumber" ("id", "guildId", "userId", "numero", "updatedAt") VALUES ('p', '10', '42', '555-0101', ?)"#,
    )
    .bind(DbTime::now())
    .execute(db)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO "Vehicle" ("id", "guildId", "plate", "ownerId", "modele", "updatedAt") VALUES ('v', '10', 'AB-123', '42', 'Sultan', ?)"#,
    )
    .bind(DbTime::now())
    .execute(db)
    .await
    .unwrap();
    let profile = members::build_profile(&app.ctx, Id::new(10), Id::new(42), true).await.unwrap();
    let value = serde_json::to_value(&profile).unwrap();
    assert_eq!(
        keys(&value),
        [
            "absences",
            "coffres",
            "cotisations",
            "joinedAt",
            "member",
            "phone",
            "presence",
            "recrue",
            "roles",
            "vehicles"
        ]
    );
    assert_eq!(value["phone"], "555-0101");
    assert_eq!(value["vehicles"], serde_json::json!([{ "plate": "AB-123", "model": "Sultan" }]));
    assert_eq!(value["member"], serde_json::json!({ "id": "42", "name": "42", "avatarUrl": null }));
    assert!(value["presence"].is_null() && value["cotisations"].is_null() && value["recrue"].is_null());
}

#[tokio::test]
async fn contact_creation_is_validated_and_journaled() {
    let app = TestApp::new(false).await;
    let ctx = &app.ctx;
    let new = contacts::new_contact(
        serde_json::from_str(r#"{"name":"Les Ballas","kind":"GROUPE","relation":"HOSTILE","phone":null}"#).unwrap(),
        "42".into(),
    )
    .unwrap();
    let row = crate::modules::contacts::service::add(&ctx.db, "10", new).await.unwrap().unwrap();
    let value = serde_json::to_value(contacts::contact(&Members::new(ctx, Id::new(10)), &row, None)).unwrap();
    assert_eq!(
        keys(&value),
        ["createdAt", "createdBy", "description", "id", "kind", "name", "notesCount", "phone", "relation", "updatedAt"]
    );
    assert_eq!((&value["kind"], &value["relation"]), (&serde_json::json!("GROUPE"), &serde_json::json!("HOSTILE")));
    assert!(value["createdAt"].as_str().unwrap().ends_with('Z'));

    audit::record_web(
        ctx,
        Id::new(10),
        Id::new(42),
        "Elodie",
        audit::Entry::new(audit::Action::ContactAdd, None, "**Les Ballas**"),
    )
    .await;
    let (rows, total) = audit::recent(&ctx.db, "10", None, 10, 0).await.unwrap();
    assert_eq!((total, rows[0].actor_id.as_str(), rows[0].action.as_str()), (1, "42", "contact.add"));
}

#[tokio::test]
async fn overview_live_and_health_need_the_guild() {
    let app = TestApp::new(true).await;
    for uri in ["/api/guilds/123/overview", "/api/guilds/123/events", "/api/guilds/123/health"] {
        let (status, _, body) = app.send(get(uri)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
        assert!(body["error"].is_string(), "{uri}");
        let request = axum::http::Request::get(uri).header(header::COOKIE, app.cookie()).body(Body::empty()).unwrap();
        let (status, _, _) = app.send(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
    }
}

#[tokio::test]
async fn journal_entries_are_announced_live() {
    let app = TestApp::new(false).await;
    let mut receiver = app.ctx.events.subscribe();
    let entry = audit::Entry::new(audit::Action::Config, None, "**coffre** — alertes");
    audit::record_web(&app.ctx, Id::new(10), Id::new(42), "Elodie", entry).await;
    let topics = [receiver.recv().await.unwrap(), receiver.recv().await.unwrap()];
    assert!(topics.iter().all(|e| e.is_for("10")));
    assert_eq!(topics.map(|e| e.topic.as_str()), ["journal", "config"]);
}

#[tokio::test]
async fn serves_the_react_interface() {
    let app = TestApp::new(true).await;
    let send_raw = |uri: &str| {
        let ctx = app.ctx.clone();
        let request = get(uri);
        async move {
            let response = crate::web::app(ctx).oneshot(request).await.unwrap();
            let status = response.status();
            let headers = response.headers().clone();
            let bytes = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
            (status, headers, String::from_utf8_lossy(&bytes).into_owned())
        }
    };

    // Interface pas encore compilée : message clair, pas de 404 muet.
    let (status, _, body) = send_raw("/").await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(body.contains("npm run build"));

    let ui = app.dir.join("ui");
    std::fs::create_dir_all(ui.join("assets")).unwrap();
    std::fs::write(ui.join("index.html"), "<!doctype html><div id=app></div>").unwrap();
    std::fs::write(ui.join("assets/index-abc123.js"), "console.log(1)").unwrap();
    std::fs::write(ui.join("theme-init.js"), "0").unwrap();
    std::fs::write(ui.join("sw.js"), "0").unwrap();
    std::fs::write(ui.join("manifest.webmanifest"), "{}").unwrap();

    // Routes de l'application (et anciennes adresses) : index.html, revalidé.
    for uri in ["/", "/serveurs/1/coffres", "/stats/1/transactions?du=2026-10-01", "/membres/5?serveur=1"] {
        let (status, headers, body) = send_raw(uri).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
        assert!(body.contains("id=app"), "{uri}");
        assert_eq!(headers[header::CACHE_CONTROL], "no-cache", "{uri}");
        assert!(headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap().contains("connect-src 'self'"));
    }
    // Fichiers avec empreinte : mis en cache longtemps, bon type.
    let (status, headers, _) = send_raw("/assets/index-abc123.js").await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers[header::CONTENT_TYPE].to_str().unwrap().starts_with("text/javascript"));
    assert!(headers[header::CACHE_CONTROL].to_str().unwrap().contains("immutable"));
    let (_, headers, _) = send_raw("/theme-init.js").await;
    assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
    // Application installable : service worker et manifeste jamais figés par le cache.
    let (_, headers, _) = send_raw("/sw.js").await;
    assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
    assert!(headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap().contains("worker-src 'self'"));
    let (_, headers, _) = send_raw("/manifest.webmanifest").await;
    assert_eq!(headers[header::CACHE_CONTROL], "no-cache");
    assert_eq!(headers[header::CONTENT_TYPE], "application/manifest+json");
    // Fichier d'assets absent : vrai 404 (pas la page à la place d'un script).
    assert_eq!(send_raw("/assets/disparu.js").await.0, StatusCode::NOT_FOUND);
    // Sortie du dossier refusée : la page de l'application, jamais le fichier.
    let (status, _, body) = send_raw("/../bot.db").await;
    assert!(status == StatusCode::OK && body.contains("id=app"));
    // L'API et la santé ne passent jamais par l'interface.
    let (status, _, body) = app.send(get("/api/inconnue")).await;
    assert_eq!((status, body["error"].is_string()), (StatusCode::NOT_FOUND, true));
    assert_eq!(send_raw("/healthz").await.2, "ok");
}

/// Serveur de démonstration pour un essai dans un navigateur :
/// `UI_SMOKE_PORT=8899 cargo test serve_for_browser -- --ignored --nocapture`
/// (sert `ui/dist`, connexion désactivée, s'arrête après 120 s).
#[tokio::test]
#[ignore = "manuel : essai de l'interface dans un navigateur"]
async fn serve_for_browser() {
    let Ok(port) = std::env::var("UI_SMOKE_PORT") else { return };
    let mut app = TestApp::new(false).await;
    let ui = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/dist");
    let ctx = Arc::get_mut(&mut app.ctx).expect("contexte unique");
    ctx.config.web.ui_dir = ui;
    let listener = tokio::net::TcpListener::bind(format!("127.0.0.1:{port}")).await.unwrap();
    let server = axum::serve(listener, crate::web::app(app.ctx.clone()));
    let _ = tokio::time::timeout(std::time::Duration::from_secs(120), server).await;
}

#[tokio::test]
async fn new_pages_refuse_without_access() {
    let app = TestApp::new(true).await;
    let gets = [
        "/api/guilds/123/presences",
        "/api/guilds/123/presences/stats?jours=90",
        "/api/guilds/123/presences/export.csv",
        "/api/guilds/123/presences/plannings",
        "/api/guilds/123/presences/abc",
        "/api/guilds/123/absences",
        "/api/guilds/123/recrues",
        "/api/guilds/123/recrues/42/history",
        "/api/guilds/123/vehicules?q=ab",
        "/api/guilds/123/numeros",
    ];
    for uri in gets {
        // Pas connecté : 401 avec le lien de connexion.
        let (status, _, body) = app.send(get(uri)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
        assert_eq!(body["loginUrl"], "/login?next=/", "{uri}");
        // Connecté, pas membre du serveur : 403 en JSON.
        let request = axum::http::Request::get(uri).header(header::COOKIE, app.cookie()).body(Body::empty()).unwrap();
        let (status, _, body) = app.send(request).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
        assert!(body["error"].as_str().unwrap().contains("serveur"), "{uri}");
    }
    let writes = [
        (Method::POST, "/api/guilds/123/presences", r#"{"time":"21:00"}"#),
        (Method::POST, "/api/guilds/123/presences/abc/close", "{}"),
        (Method::POST, "/api/guilds/123/presences/plannings", r#"{"weekday":3,"time":"21:00"}"#),
        (Method::DELETE, "/api/guilds/123/presences/plannings/abc", "{}"),
        (Method::POST, "/api/guilds/123/absences", r#"{"motif":"x","to":"2099-01-01"}"#),
        (Method::POST, "/api/guilds/123/absences/abc/end", "{}"),
        (Method::PUT, "/api/guilds/123/recrues/abc/vote", r#"{"choice":"POUR","motif":"x"}"#),
        (Method::PUT, "/api/guilds/123/recrues/abc/veto", r#"{"motif":"x"}"#),
        (Method::DELETE, "/api/guilds/123/recrues/abc/veto", "{}"),
        (Method::POST, "/api/guilds/123/vehicules", r#"{"plate":"AB12","ownerId":"42"}"#),
        (Method::DELETE, "/api/guilds/123/vehicules/AB12", "{}"),
        (Method::PUT, "/api/guilds/123/numeros/42", r#"{"number":"5551234"}"#),
        (Method::DELETE, "/api/guilds/123/numeros/42", "{}"),
        (Method::POST, "/api/guilds/123/transactions", r#"{"groupe":"x","type":"ACHAT","prix":1}"#),
        (Method::PUT, "/api/guilds/123/transactions/abc/status", r#"{"status":"LIVRE_PAYE"}"#),
        (Method::POST, "/api/guilds/123/transactions/abc/items", r#"{"nom":"x"}"#),
        (Method::POST, "/api/guilds/123/transactions/abc/cancel", "{}"),
    ];
    for (method, uri, body) in writes {
        let request = |csrf: bool| {
            let mut request =
                axum::http::Request::builder().method(method.clone()).uri(uri).header(header::COOKIE, app.cookie());
            if csrf {
                request = request.header(header::CONTENT_TYPE, "application/json").header("x-requested-with", "fetch");
            }
            request.body(Body::from(body)).unwrap()
        };
        let (status, _, refused) = app.send(request(false)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}");
        assert!(refused["error"].as_str().unwrap().contains("X-Requested-With"), "{method} {uri}");
        let (status, _, refused) = app.send(request(true)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}");
        assert!(refused["error"].as_str().unwrap().contains("serveur"), "{method} {uri}");
    }
}

#[test]
fn new_pages_follow_permission_levels() {
    let guild = Id::new(10);
    let member = access(&[], &["1"], false);
    let grade = access(&[], &["1", "2"], false);
    let lead = access(&[], &["1", "3"], false);
    let admin = access(&[], &[], true);
    let outsider = access(&[], &["9"], false);

    // Présences : lecture et création ouvertes au groupe, plannings aux gradés.
    assert!(check_guild(&member, guild, Some(Perm::PresenceStats)).is_ok());
    assert!(check_guild(&member, guild, Some(Perm::PresenceCreer)).is_ok());
    assert!(check_guild(&member, guild, Some(Perm::PresenceGerer)).is_err());
    assert!(check_guild(&grade, guild, Some(Perm::PresenceGerer)).is_ok());
    assert!(check_guild(&outsider, guild, Some(Perm::PresenceStats)).is_err());

    // Absences et numéros : membres du groupe (« Gérer le serveur » pour administrer).
    assert!(absences::check_group(&member, guild).is_ok());
    assert!(absences::check_group(&admin, guild).is_ok());
    let refused = absences::check_group(&outsider, guild).unwrap_err();
    assert_eq!(refused.status(), StatusCode::FORBIDDEN);
    assert!(refused.message().contains("membres du groupe"));
    assert!(absences::check_group(&member, Id::new(11)).is_err());
    assert!(check_guild(&member, guild, Some(Perm::AbsenceAutrui)).is_err());
    assert!(check_guild(&grade, guild, Some(Perm::AbsenceAutrui)).is_ok());

    // Recrues : actions strictes, « Gérer le serveur » n'y donne pas accès.
    assert!(check_guild(&admin, guild, Some(Perm::RecrueAvis)).is_err());
    assert!(check_guild(&admin, guild, Some(Perm::RecrueVeto)).is_err());
    assert!(check_guild(&grade, guild, Some(Perm::RecrueAvis)).is_ok());
    assert!(check_guild(&grade, guild, Some(Perm::RecrueVeto)).is_err());
    assert!(check_guild(&lead, guild, Some(Perm::RecrueVeto)).is_ok());
    assert!(check_guild(&member, guild, Some(Perm::RecrueSuivi)).is_err());

    // Annuaire et transactions.
    assert!(check_guild(&member, guild, Some(Perm::Vehicule)).is_ok());
    assert!(check_guild(&member, guild, Some(Perm::TransactionUtiliser)).is_ok());
    assert!(check_guild(&member, guild, Some(Perm::NumeroSupprimerAutrui)).is_err());
    assert!(check_guild(&grade, guild, Some(Perm::NumeroAjouterAutrui)).is_err());
    assert!(check_guild(&admin, guild, Some(Perm::NumeroAjouterAutrui)).is_ok());
}

#[tokio::test]
async fn schedules_are_saved_and_removed_per_guild() {
    use crate::modules::presences::planning;
    let app = TestApp::new(false).await;
    let db = &app.ctx.db;
    let input: presences::NewScheduleInput =
        serde_json::from_str(r#"{"weekday":3,"time":"21h00","title":"Braquage","leadHours":48}"#).unwrap();
    let schedule = presences::check_schedule(input, "10", "42").unwrap();
    planning::insert(db, &schedule).await.unwrap();
    let list = planning::list(db, "10").await.unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!((list[0].time.as_str(), list[0].lead_hours, list[0].weekday_label()), ("21:00", 48, "mercredi"));
    // Un autre serveur ne peut pas le supprimer.
    assert!(!planning::delete(db, "11", &schedule.id).await.unwrap());
    assert!(planning::delete(db, "10", &schedule.id).await.unwrap());
    assert!(planning::list(db, "10").await.unwrap().is_empty());
}
