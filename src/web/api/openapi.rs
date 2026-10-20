//! Spécification OpenAPI 3.1 de l'API (`GET /api/openapi.json`), d'où
//! l'interface génère son client TypeScript. Les formes suivent
//! `ui/src/api/types.ts` ; le fichier `ui/openapi.json` est vérifié par un test
//! (`UPDATE_OPENAPI=1 cargo test openapi_spec_is_up_to_date` pour le régénérer).

use super::{
    MemberRef, RoleRef, absences, annuaire, coffres, contacts, cotisations, journal, me, members, permissions,
    presences, recrues, stats, tickets, transaction_edits, transactions,
};
use crate::modules::permissions::{Level, Perm};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use utoipa::{
    OpenApi, PartialSchema, ToSchema,
    openapi::{
        RefOr, Schema,
        schema::{ObjectBuilder, SchemaType, Type},
    },
};

/// Chaîne limitée à des valeurs fixes (union de littéraux côté TypeScript).
fn string_enum<I: IntoIterator<Item = &'static str>>(description: &str, values: I) -> RefOr<Schema> {
    ObjectBuilder::new()
        .schema_type(SchemaType::Type(Type::String))
        .description(Some(description))
        .enum_values(Some(values))
        .into()
}

macro_rules! string_enum {
    ($name:ident, $description:literal, [$($value:literal),+ $(,)?]) => {
        #[doc = $description]
        pub struct $name;

        impl PartialSchema for $name {
            fn schema() -> RefOr<Schema> {
                string_enum($description, [$($value),+])
            }
        }

        impl ToSchema for $name {
            fn name() -> Cow<'static, str> {
                Cow::Borrowed(stringify!($name))
            }
        }
    };
}

/// Clé d'action soumise à permission (`perms!`).
pub struct PermKey;

impl PartialSchema for PermKey {
    fn schema() -> RefOr<Schema> {
        string_enum("Clé d'action soumise à permission.", Perm::ALL.iter().map(|p| p.key()))
    }
}

impl ToSchema for PermKey {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("Perm")
    }
}

/// Niveau de permission.
pub struct LevelKey;

impl PartialSchema for LevelKey {
    fn schema() -> RefOr<Schema> {
        string_enum("Niveau de permission, du plus ouvert au plus restreint.", Level::ALL.iter().map(|l| l.as_str()))
    }
}

impl ToSchema for LevelKey {
    fn name() -> Cow<'static, str> {
        Cow::Borrowed("Level")
    }
}

string_enum!(PresenceStatus, "Réponse à une présence.", ["PRESENT", "ABSENT", "RETARD", "NON_REPONDU"]);
string_enum!(TransactionType, "Sens d'une transaction.", ["ACHAT", "VENTE"]);
string_enum!(
    TransactionStatus,
    "Statut d'une transaction.",
    ["PREPARATION", "EN_ATTENTE_RDV", "AVANCE_ATTENTE_PAIEMENT", "LIVRE_PAYE", "ANNULEE", "CLOTUREE"]
);
string_enum!(CoffreKind, "Dépôt, retrait ou correction d'inventaire.", ["DEPOSIT", "WITHDRAW", "ADJUST"]);
string_enum!(ContactKind, "Nature d'un contact.", ["GROUPE", "ENTREPRISE", "PERSONNE"]);
string_enum!(Relation, "Relation d'un contact avec le groupe.", ["ALLIE", "NEUTRE", "HOSTILE"]);
string_enum!(TicketStatus, "Statut d'un ticket.", ["OPEN", "CLOSED", "SUSPENDED"]);
string_enum!(LogKind, "Type d'un message de log.", ["user", "staff", "internal", "command", "system", "danger"]);
string_enum!(ObjectiveCounts, "Ce qui compte pour l'objectif.", ["total", "propre", "sale"]);
string_enum!(ModuleStatus, "État d'un module.", ["ok", "incomplete", "off"]);
string_enum!(RecrueChoice, "Avis sur une recrue.", ["POUR", "CONTRE", "BLANC"]);
string_enum!(PresenceAnswer, "Réponse donnée à une présence.", ["PRESENT", "ABSENT", "RETARD"]);

/// Schéma d'une page (`Page<T>` du contrat) : un type nommé par liste, pour que
/// les éléments restent des références (`Transaction`…) dans le client généré.
macro_rules! page_schema {
    ($name:ident, $item:ty) => {
        #[derive(ToSchema)]
        #[serde(rename_all = "camelCase")]
        #[allow(dead_code)]
        pub struct $name {
            items: Vec<$item>,
            total: i64,
            /// 1 = première page.
            page: i64,
            page_size: i64,
        }
    };
}

page_schema!(TransactionPage, transactions::Transaction);
page_schema!(CotisationEntryPage, cotisations::CotisationEntry);
page_schema!(CoffreMovementPage, coffres::CoffreMovement);
page_schema!(JournalEntryPage, journal::JournalEntry);
page_schema!(TicketPage, tickets::TicketRow);
page_schema!(PresenceSummaryPage, presences::Summary);

// ---------------------------------------------------------------------------
// Schémas des routes encore en attente (501), à brancher par l'intégration.

/// Objectif hebdomadaire (lundi → dimanche, heure de Paris).
#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Objective {
    pub amount: i64,
    #[schema(value_type = ObjectiveCounts)]
    pub counts: String,
    /// Rappel le dimanche aux membres en dessous de l'objectif.
    pub reminder: bool,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveProgress {
    pub member: MemberRef,
    pub amount: i64,
    pub target: i64,
    pub reached: bool,
    /// Absence déclarée ou recrue : non concerné cette semaine.
    pub exempt: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CoffreThreshold {
    /// `null` : toutes les propriétés.
    #[schema(required = true)]
    pub property_id: Option<String>,
    pub item: String,
    pub min: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CoffreAlerts {
    /// Salon où publier les alertes, `null` : désactivées.
    #[schema(required = true)]
    pub channel_id: Option<String>,
    /// Rôle mentionné dans les alertes.
    #[schema(required = true)]
    pub role_id: Option<String>,
    pub thresholds: Vec<CoffreThreshold>,
    /// Un retrait unique au-dessus de cette quantité déclenche une alerte (`null` : désactivé).
    #[schema(required = true)]
    pub big_withdrawal: Option<i64>,
    /// Alerte quand un personnage relié à aucun membre retire quelque chose.
    pub unknown_player: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigCheck {
    pub label: String,
    pub ok: bool,
    #[schema(required = true)]
    pub detail: Option<String>,
    /// Commande Discord pour corriger.
    #[schema(required = true)]
    pub fix: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigModule {
    pub key: String,
    pub label: String,
    #[schema(value_type = ModuleStatus)]
    pub status: String,
    pub checks: Vec<ConfigCheck>,
}

#[derive(Clone, Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigStatus {
    pub modules: Vec<ConfigModule>,
}

// ---------------------------------------------------------------------------

#[derive(OpenApi)]
#[openapi(
    info(
        title = "API du bot",
        version = "1.0.0",
        description = "API JSON du site (`/api`). Écritures : en-têtes `Content-Type: application/json` et `X-Requested-With: fetch` obligatoires."
    ),
    paths(
        me::me,
        members::search,
        members::profile,
        stats::stats,
        transactions::list,
        transactions::export,
        cotisations::list,
        cotisations::export,
        cotisations::create,
        cotisations::remove,
        super::reglages::objective_get,
        super::reglages::objective_put,
        coffres::meta,
        coffres::stock,
        coffres::movements,
        coffres::players,
        coffres::series,
        coffres::export,
        super::reglages::alerts_get,
        super::reglages::alerts_put,
        coffres::name_property,
        coffres::link_player,
        coffres::inventory,
        coffres::unlink_player,
        contacts::list,
        contacts::detail,
        contacts::create,
        contacts::update,
        contacts::remove,
        contacts::add_note,
        journal::journal,
        permissions::show,
        permissions::update,
        super::reglages::config_get,
        presences::list,
        presences::detail,
        presences::stats,
        presences::export,
        presences::create,
        presences::close,
        presences::schedules,
        presences::create_schedule,
        presences::delete_schedule,
        absences::list,
        absences::create,
        absences::end,
        recrues::list,
        recrues::history,
        recrues::vote,
        recrues::set_veto,
        recrues::clear_veto,
        annuaire::vehicles,
        annuaire::create_vehicle,
        annuaire::delete_vehicle,
        annuaire::phones,
        annuaire::set_phone,
        annuaire::delete_phone,
        transaction_edits::create,
        transaction_edits::set_status,
        transaction_edits::add_item,
        transaction_edits::cancel,
        tickets::list,
        tickets::staff,
        tickets::log,
    ),
    components(schemas(PermKey, LevelKey, RoleRef, Objective, ObjectiveProgress, CoffreAlerts, ConfigStatus)),
    tags(
        (name = "session", description = "Connexion et droits"),
        (name = "membres", description = "Membres et fiches"),
        (name = "stats", description = "Statistiques du serveur"),
        (name = "transactions", description = "Historique des transactions"),
        (name = "cotisations", description = "Cotisations"),
        (name = "coffres", description = "Coffres des propriétés"),
        (name = "contacts", description = "Répertoire des contacts"),
        (name = "journal", description = "Journal du staff"),
        (name = "reglages", description = "Permissions et état de la configuration"),
        (name = "tickets", description = "Tickets (module ticket)"),
        (name = "presences", description = "Présences, assiduité et plannings"),
        (name = "absences", description = "Absences déclarées"),
        (name = "recrues", description = "Avis et vétos sur les recrues"),
        (name = "annuaire", description = "Véhicules et numéros"),
    )
)]
pub struct ApiDoc;

/// Spécification en JSON lisible (ordre stable : clés triées).
pub fn spec_json() -> String {
    let value = serde_json::to_value(ApiDoc::openapi()).expect("spécification sérialisable");
    let mut text = serde_json::to_string_pretty(&value).expect("spécification sérialisable");
    text.push('\n');
    text
}

/// `GET /api/openapi.json` : public, la spécification ne contient aucune donnée.
pub async fn serve() -> Response {
    ([(axum::http::header::CONTENT_TYPE, "application/json; charset=utf-8")], spec_json()).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn openapi_spec_is_up_to_date() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("ui/openapi.json");
        let spec = spec_json();
        if std::env::var("UPDATE_OPENAPI").is_ok_and(|v| v == "1") {
            std::fs::write(&path, &spec).unwrap();
            return;
        }
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        assert!(
            current == spec,
            "ui/openapi.json n'est plus à jour : UPDATE_OPENAPI=1 cargo test openapi_spec_is_up_to_date"
        );
    }

    #[test]
    fn spec_follows_the_contract() {
        let spec: serde_json::Value = serde_json::from_str(&spec_json()).unwrap();
        assert_eq!(spec["openapi"], "3.1.0");
        let operation = &spec["paths"]["/api/guilds/{guild}/cotisations"]["get"];
        assert_eq!(operation["operationId"], "getCotisations");
        let schemas = &spec["components"]["schemas"];
        assert_eq!(schemas["Perm"]["enum"][0], "presence.creer");
        assert_eq!(schemas["Level"]["enum"], serde_json::json!(["membre", "grade", "lead", "admin"]));
        assert!(schemas["ApiError"]["required"] == serde_json::json!(["error"]));
    }
}
