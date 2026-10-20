//! État de la configuration : pour chaque module, ce qui est réglé, ce qui
//! manque et la commande Discord pour le corriger. Affiché par `/config etat`
//! et servi au site (`ConfigStatus` côté TypeScript).

use crate::{
    context::Ctx,
    discord::{
        embed::{EmbedBuilder, footer},
        interaction::{GUILD_ONLY, Ix, MANAGE_GUILD_ONLY, Reply},
        messages::parse_id,
    },
    modules::{
        coffres, contacts, cotisations, permissions, phones, presences, rappels, recrues, transactions, vehicles,
    },
    util::format::{field, truncate},
};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use twilight_model::{
    guild::Permissions,
    id::{
        Id,
        marker::{ChannelMarker, GuildMarker, RoleMarker},
    },
};
use twilight_util::builder::command::SubCommandBuilder;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Prêt.
    Ok,
    /// Il manque quelque chose.
    Incomplete,
    /// Module non utilisé.
    Off,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    pub label: String,
    pub ok: bool,
    pub detail: Option<String>,
    /// Commande Discord pour corriger.
    pub fix: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleStatus {
    pub key: String,
    pub label: String,
    pub status: Status,
    pub checks: Vec<Check>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigStatus {
    pub modules: Vec<ModuleStatus>,
}

/// Réglage requis : manquant, il rend le module incomplet.
fn required(label: &str, ok: bool, detail: Option<String>, fix: &str) -> Check {
    Check { label: label.to_owned(), ok, detail, fix: (!ok).then(|| fix.to_owned()) }
}

/// Réglage facultatif : toujours « ok », la commande est indiquée s'il n'est pas fait.
fn optional(label: &str, set: bool, unset_detail: &str, fix: &str) -> Check {
    Check {
        label: label.to_owned(),
        ok: true,
        detail: (!set).then(|| unset_detail.to_owned()),
        fix: (!set).then(|| fix.to_owned()),
    }
}

/// Module : désactivé s'il n'est pas utilisé, sinon prêt si tous les contrôles passent.
fn module(key: &str, label: &str, used: bool, checks: Vec<Check>) -> ModuleStatus {
    let status = if !used {
        Status::Off
    } else if checks.iter().all(|c| c.ok) {
        Status::Ok
    } else {
        Status::Incomplete
    };
    ModuleStatus { key: key.to_owned(), label: label.to_owned(), status, checks }
}

/// Un salon réglé : existe-t-il encore ? `None` : pas réglé.
fn channel_state(ctx: &Ctx, id: Option<&str>) -> Option<Result<(), String>> {
    let id = id?;
    Some(match parse_id::<ChannelMarker>(id) {
        Some(channel) if ctx.cache.channel(channel).is_some() => Ok(()),
        _ => Err(format!("Salon <#{id}> introuvable (supprimé, ou invisible pour le bot).")),
    })
}

fn role_state(ctx: &Ctx, id: Option<&str>) -> Option<Result<(), String>> {
    let id = id?;
    Some(match parse_id::<RoleMarker>(id) {
        Some(role) if ctx.cache.role(role).is_some() => Ok(()),
        _ => Err(format!("Rôle {id} introuvable (supprimé ?).")),
    })
}

/// Contrôle d'un salon ou d'un rôle requis.
fn required_ref(label: &str, state: Option<Result<(), String>>, missing: &str, fix: &str) -> Check {
    match state {
        Some(Ok(())) => required(label, true, None, fix),
        Some(Err(detail)) => required(label, false, Some(detail), fix),
        None => required(label, false, Some(missing.to_owned()), fix),
    }
}

/// Contrôle d'un salon ou d'un rôle facultatif : en erreur seulement s'il est réglé mais introuvable.
fn optional_ref(label: &str, state: Option<Result<(), String>>, unset: &str, fix: &str) -> Check {
    match state {
        Some(Ok(())) => optional(label, true, unset, fix),
        Some(Err(detail)) => required(label, false, Some(detail), fix),
        None => optional(label, false, unset, fix),
    }
}

pub async fn config_status(ctx: &Ctx, guild_id: Id<GuildMarker>) -> Result<ConfigStatus> {
    let guild = guild_id.to_string();
    let db = &ctx.db;
    let mut modules = Vec::new();

    // Présences et absences.
    let presence = presences::service::get_config(db, &guild).await?;
    let p = |f: fn(&presences::model::PresenceConfig) -> Option<String>| presence.as_ref().and_then(f);
    let role = p(|c| c.default_role_id.clone());
    modules.push(module(
        "presences",
        "Présences",
        role.is_some(),
        vec![
            required_ref(
                "Rôle ciblé configuré",
                role_state(ctx, role.as_deref()),
                "Aucun rôle : personne n'est pingé ni compté.",
                "/config presence role:@…",
            ),
            optional_ref(
                "Salon des présences",
                channel_state(ctx, p(|c| c.presence_channel_id.clone()).as_deref()),
                "Non défini : publiées dans le salon de la commande.",
                "/config presence channel:#…",
            ),
            optional_ref(
                "Salon des rappels",
                channel_state(ctx, p(|c| c.reminder_channel_id.clone()).as_deref()),
                "Non défini : rappels dans le salon de la présence.",
                "/config presence reminder_channel:#…",
            ),
            optional_ref(
                "Rapport hebdomadaire d'assiduité",
                channel_state(ctx, p(|c| c.report_channel_id.clone()).as_deref()),
                "Pas de rapport le lundi.",
                "/config presence rapport_salon:#…",
            ),
        ],
    ));
    let absence_role = p(|c| c.absence_role_id.clone());
    let absence_channel = p(|c| c.absence_channel_id.clone());
    modules.push(module(
        "absences",
        "Absences",
        absence_role.is_some() || absence_channel.is_some(),
        vec![
            required_ref(
                "Rôle d'absence",
                role_state(ctx, absence_role.as_deref()),
                "Aucun rôle posé pendant les absences.",
                "/config presence absence_role:@…",
            ),
            optional_ref(
                "Salon des absences",
                channel_state(ctx, absence_channel.as_deref()),
                "Les absences ne sont annoncées nulle part.",
                "/config presence absence_channel:#…",
            ),
        ],
    ));

    // Transactions, véhicules, numéros : un tableau de bord.
    let transaction = transactions::service::get_config(db, &guild).await?;
    let dashboard = transaction.as_ref().and_then(|c| c.dashboard_channel_id.clone());
    modules.push(module(
        "transactions",
        "Transactions",
        dashboard.is_some(),
        vec![
            required_ref(
                "Tableau de bord",
                channel_state(ctx, dashboard.as_deref()),
                "Pas de tableau de bord des transactions.",
                "/config transaction dashboard_channel:#…",
            ),
            optional_ref(
                "Rôle mentionné à la création",
                role_state(ctx, transaction.as_ref().and_then(|c| c.ping_role_id.as_deref())),
                "Aucun rôle mentionné.",
                "/config transaction ping_role:@…",
            ),
        ],
    ));
    let vehicle = vehicles::service::get_config(db, &guild).await?.and_then(|c| c.dashboard_channel_id);
    modules.push(module(
        "vehicules",
        "Véhicules",
        vehicle.is_some(),
        vec![required_ref(
            "Tableau de bord",
            channel_state(ctx, vehicle.as_deref()),
            "Pas de tableau de bord des véhicules.",
            "/config vehicule dashboard_channel:#…",
        )],
    ));
    let phone = phones::service::get_config(db, &guild).await?.and_then(|c| c.dashboard_channel_id);
    modules.push(module(
        "numeros",
        "Numéros",
        phone.is_some(),
        vec![required_ref(
            "Annuaire",
            channel_state(ctx, phone.as_deref()),
            "Pas d'annuaire publié.",
            "/config numero salon:#…",
        )],
    ));

    // Recrues.
    let recrue = recrues::service::get_config(db, &guild).await?;
    let r = |f: fn(&recrues::service::RecrueConfig) -> Option<String>| recrue.as_ref().and_then(f);
    let recrue_role = r(|c| c.recrue_role_id.clone());
    let grade_role = r(|c| c.grade_role_id.clone());
    modules.push(module(
        "recrues",
        "Recrues",
        recrue_role.is_some() || grade_role.is_some(),
        vec![
            required_ref(
                "Rôle recrue",
                role_state(ctx, recrue_role.as_deref()),
                "Aucun rôle : personne n'est suivi.",
                "/config recrue role_recrue:@…",
            ),
            required_ref(
                "Rôle gradé",
                role_state(ctx, grade_role.as_deref()),
                "Personne ne peut donner son avis.",
                "/config recrue role_grade:@…",
            ),
            optional_ref(
                "Rôle lead",
                role_state(ctx, r(|c| c.lead_role_id.clone()).as_deref()),
                "Personne ne peut poser de véto.",
                "/config recrue role_lead:@…",
            ),
            optional_ref(
                "Catégorie des salons d'essai",
                channel_state(ctx, r(|c| c.category_id.clone()).as_deref()),
                "Pas de salon d'essai créé.",
                "/config recrue categorie:#…",
            ),
            optional_ref(
                "Salon des avis",
                channel_state(ctx, r(|c| c.dashboard_channel_id.clone()).as_deref()),
                "Pas de message d'avis de la semaine.",
                "/config recrue salon:#…",
            ),
        ],
    ));

    // Cotisations.
    let announce = cotisations::service::channel(db, &guild).await?;
    let objective = cotisations::objective::get_objective(db, &guild).await?;
    let (board, _) = cotisations::dashboard::get_dashboard(db, &guild).await?;
    let mut checks = vec![optional_ref(
        "Salon des annonces",
        channel_state(ctx, announce.as_deref()),
        "Les cotisations ne sont pas annoncées.",
        "/config cotisation salon:#…",
    )];
    match &objective {
        Some(objective) => {
            let tracked = cotisations::objective::tracked_role(db, &guild).await?;
            checks.push(required("Objectif hebdomadaire", true, Some(objective.describe()), ""));
            checks.push(required_ref(
                "Membres suivis",
                role_state(ctx, tracked.as_deref()),
                "Aucun rôle membre ni rôle des présences : personne n'est suivi.",
                "/config permission role_membre:@…",
            ));
            if objective.reminder {
                checks.push(required(
                    "Salon du rappel et du bilan",
                    announce.is_some() || board.is_some(),
                    (announce.is_none() && board.is_none())
                        .then(|| "Ni salon des annonces ni tableau : rappel et bilan ne partent pas.".to_owned()),
                    "/config cotisation salon:#…",
                ));
            }
        }
        None => checks.push(optional(
            "Objectif hebdomadaire",
            false,
            "Pas d'objectif.",
            "/config cotisation objectif:… compte:total rappel:true",
        )),
    }
    checks.push(optional_ref(
        "Tableau de bord",
        channel_state(ctx, board.as_deref()),
        "Pas de tableau permanent.",
        "/config cotisation tableau:#…",
    ));
    modules.push(module(
        "cotisations",
        "Cotisations",
        announce.is_some() || objective.is_some() || board.is_some(),
        checks,
    ));

    // Rappels en MP : l'objectif doit exister pour son rappel.
    let reminders = rappels::service::get_config(db, &guild).await?;
    let mut checks = vec![optional(
        "Rappels de présence",
        reminders.enabled,
        "Désactivés : aucun MP avant les présences.",
        "/config rappels actif:true",
    )];
    if reminders.enabled && reminders.objective {
        checks.push(optional(
            "Rappel de l'objectif",
            objective.is_some(),
            "Pas d'objectif de cotisation : pas de MP le samedi.",
            "/config cotisation objectif:…",
        ));
    }
    modules.push(module("rappels", "Rappels en MP", reminders.enabled, checks));

    // Coffres.
    let channels = coffres::service::channels(db, &guild).await?;
    let alerts = coffres::alerts::get_alerts(db, &guild).await?;
    let (stock_board, _) = coffres::dashboard::get_dashboard(db, &guild).await?;
    let mut checks = vec![
        required(
            "Message Content Intent",
            ctx.message_content,
            (!ctx.message_content).then(|| "Le bot ne peut pas lire les encadrés du jeu.".to_owned()),
            "Portail développeur Discord → Bot → Message Content Intent, puis redémarrer le bot",
        ),
        required(
            "Salon du webhook suivi",
            !channels.is_empty(),
            channels.is_empty().then(|| "Aucun salon suivi.".to_owned()),
            "/config coffre salon:#…",
        ),
    ];
    for channel in &channels {
        if let Some(Err(detail)) = channel_state(ctx, Some(&channel.channel_id)) {
            checks.push(required(
                "Salon suivi accessible",
                false,
                Some(detail),
                "/config coffre salon:#… retirer:true",
            ));
        }
    }
    if !channels.is_empty() {
        let unknown: Vec<String> =
            channels.iter().filter(|c| c.webhook_id.is_none()).map(|c| format!("<#{}>", c.channel_id)).collect();
        checks.push(required(
            "Webhook du jeu reconnu",
            unknown.is_empty(),
            (!unknown.is_empty()).then(|| format!("Aucun encadré « Coffre » encore vu dans {}.", unknown.join(", "))),
            "Attendre un premier mouvement publié par le jeu, ou /config coffre salon:#…",
        ));
    }
    checks.push(optional_ref(
        "Alertes",
        channel_state(ctx, alerts.channel_id.as_deref()),
        "Pas d'alerte (stock bas, gros retraits…).",
        "/config coffre alertes_salon:#…",
    ));
    if alerts.channel_id.is_some() {
        let anything = !alerts.thresholds.is_empty() || alerts.big_withdrawal.is_some() || alerts.unknown_player;
        checks.push(required(
            "Déclencheurs d'alerte",
            anything,
            (!anything).then(|| "Ni seuil, ni gros retrait, ni personnage inconnu : aucune alerte ne partira.".into()),
            "/coffre seuil objet:… min:…",
        ));
    }
    checks.push(optional_ref(
        "Tableau du stock",
        channel_state(ctx, stock_board.as_deref()),
        "Pas de tableau permanent.",
        "/config coffre tableau:#…",
    ));
    modules.push(module("coffres", "Coffres", !channels.is_empty(), checks));

    // Contacts.
    let contact = contacts::service::get_config(db, &guild).await?;
    modules.push(module(
        "contacts",
        "Contacts",
        contact.channel_id.is_some(),
        vec![optional_ref(
            "Répertoire publié",
            channel_state(ctx, contact.channel_id.as_deref()),
            "Pas de répertoire permanent (`/contact liste` reste disponible).",
            "/config contact salon:#…",
        )],
    ));

    // Journal du staff.
    let audit: Option<(Option<String>,)> =
        sqlx::query_as(r#"SELECT "auditChannelId" FROM "GuildSettings" WHERE "guildId" = ?"#)
            .bind(&guild)
            .fetch_optional(db)
            .await?;
    let audit = audit.and_then(|(channel,)| channel);
    modules.push(module(
        "journal",
        "Journal du staff",
        audit.is_some(),
        vec![optional_ref(
            "Salon du journal",
            channel_state(ctx, audit.as_deref()),
            "Journal consultable sur le site seulement.",
            "/config journal salon:#…",
        )],
    ));

    // Permissions : toujours en service. Le rôle membre est le passage obligé.
    let roles = permissions::service::roles(db, &guild).await?;
    modules.push(module(
        "permissions",
        "Permissions",
        true,
        vec![
            required_ref(
                "Rôle membre",
                role_state(ctx, roles.member.as_deref()),
                "Indispensable : sans rôle membre, seuls les gradés et leads peuvent utiliser le bot, et rien n'est enregistré pour les autres.",
                "/config permission role_membre:@…",
            ),
            required_ref(
                "Rôle gradé",
                role_state(ctx, roles.grade.as_deref()),
                "Seuls « Gérer les messages » et « Gérer le serveur » comptent comme gradés.",
                "/config recrue role_grade:@…",
            ),
            optional_ref(
                "Rôle lead",
                role_state(ctx, roles.lead.as_deref()),
                "Personne n'a le niveau lead.",
                "/config recrue role_lead:@…",
            ),
        ],
    ));

    // Tickets : seulement si le module est configuré.
    if let Some(modmail) = &ctx.modmail {
        let config = &modmail.config;
        let linked = config.main_server_ids.contains(&guild_id) || config.inbox_server_id == guild_id;
        let mut checks = vec![required(
            "Serveur relié au module ticket",
            linked,
            (!linked).then(|| "Ce serveur n'est ni un serveur principal ni le serveur des tickets.".to_owned()),
            "mainServerId / inboxServerId dans config.ini",
        )];
        if config.inbox_server_id == guild_id {
            checks.push(required(
                "Salon des logs",
                ctx.cache.channel(config.log_channel_id).is_some(),
                ctx.cache.channel(config.log_channel_id).is_none().then(|| "Salon des logs introuvable.".to_owned()),
                "logChannelId dans config.ini",
            ));
        }
        modules.push(module("tickets", "Tickets", linked, checks));
    }

    // Le rôle membre conditionne tout le reste : les permissions en premier.
    if let Some(index) = modules.iter().position(|m| m.key == "permissions") {
        let permissions = modules.remove(index);
        modules.insert(0, permissions);
    }
    Ok(ConfigStatus { modules })
}

/// `/config etat`.
pub fn config_subcommand() -> SubCommandBuilder {
    SubCommandBuilder::new("etat", "État de la configuration de chaque module, et commandes pour la compléter")
}

fn status_icon(status: Status) -> &'static str {
    match status {
        Status::Ok => "✅",
        Status::Incomplete => "⚠️",
        Status::Off => "⚪",
    }
}

/// Champ d'un module : contrôles et commandes pour corriger.
fn module_field(module: &ModuleStatus) -> String {
    module
        .checks
        .iter()
        .map(|check| {
            let icon = if !check.ok {
                "❌"
            } else if check.detail.is_some() {
                "▫️"
            } else {
                "✅"
            };
            let mut line = format!("{icon} {}", check.label);
            if let Some(detail) = &check.detail {
                line.push_str(&format!(" — {detail}"));
            }
            if let Some(fix) = &check.fix
                && !fix.is_empty()
            {
                line.push_str(&format!(" · `{fix}`"));
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub async fn command(ix: &Ix) -> Result<()> {
    let Some(guild_id) = ix.guild_id() else {
        return ix.reply(Reply::ephemeral(GUILD_ONLY)).await;
    };
    if !ix.has_permission(Permissions::MANAGE_GUILD) {
        return ix.reply(Reply::ephemeral(MANAGE_GUILD_ONLY)).await;
    }
    let status = config_status(&ix.ctx, guild_id).await?;
    let count = |s: Status| status.modules.iter().filter(|m| m.status == s).count();
    let description = format!(
        "✅ {} prêt(s) · ⚠️ {} incomplet(s) · ⚪ {} non utilisé(s)\n-# ▫️ : facultatif, non réglé.",
        count(Status::Ok),
        count(Status::Incomplete),
        count(Status::Off)
    );
    // Les modules à compléter d'abord.
    let mut modules: Vec<&ModuleStatus> = status.modules.iter().collect();
    modules.sort_by_key(|m| match m.status {
        Status::Incomplete => 0,
        Status::Ok => 1,
        Status::Off => 2,
    });
    let mut embed = EmbedBuilder::new().title("⚙️ État de la configuration").color(0x5865f2).description(description);
    // Un embed ne dépasse pas 6 000 caractères au total : 5 200 répartis entre les modules.
    let budget = (5200 / modules.len().max(1)).min(450);
    for module in modules {
        embed = embed.field(field(
            format!("{} {}", status_icon(module.status), module.label),
            truncate(&module_field(module), budget),
            false,
        ));
    }
    let embed = embed.footer(footer("Réglages : /config <module>")).build();
    ix.reply(Reply::embed(embed).hidden()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_status_follows_checks() {
        let ok = required("a", true, None, "/x");
        let missing = required("b", false, Some("manque".into()), "/y");
        let opt = optional("c", false, "facultatif", "/z");
        assert_eq!(module("k", "K", true, vec![ok.clone(), opt.clone()]).status, Status::Ok);
        assert_eq!(module("k", "K", true, vec![ok.clone(), missing.clone()]).status, Status::Incomplete);
        assert_eq!(module("k", "K", false, vec![missing.clone()]).status, Status::Off);
        assert_eq!(ok.fix, None);
        assert_eq!(missing.fix.as_deref(), Some("/y"));
        assert!(opt.ok && opt.fix.as_deref() == Some("/z"));
        let text = module_field(&module("k", "K", true, vec![ok, missing, opt]));
        assert_eq!(text, "✅ a\n❌ b — manque · `/y`\n▫️ c — facultatif · `/z`");
    }

    #[test]
    fn serializes_like_the_typescript_contract() {
        let status = ConfigStatus {
            modules: vec![module("coffres", "Coffres", true, vec![required("Webhook", false, None, "/config coffre")])],
        };
        assert_eq!(
            serde_json::to_value(&status).unwrap(),
            serde_json::json!({ "modules": [{
                "key": "coffres", "label": "Coffres", "status": "incomplete",
                "checks": [{ "label": "Webhook", "ok": false, "detail": null, "fix": "/config coffre" }]
            }] })
        );
    }
}
