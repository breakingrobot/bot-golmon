//! Permissions par action. Chaque action (`contact.ajouter`, `coffre.voir`…) a
//! un niveau : tout le monde, membres du groupe, gradés, leads ou « Gérer le
//! serveur ». Les niveaux s'emboîtent (un lead est aussi gradé et membre) et
//! chaque serveur peut changer celui d'une action avec `/config permission`.
//!
//! Rôles : « membre » est réglé ici, « gradé » et « lead » viennent du module
//! recrues. « Gérer les messages » compte comme gradé, et « Gérer le serveur »
//! ouvre tout, sauf les actions strictes (avis et vétos des recrues), réservées
//! aux rôles.

pub mod commands;
pub mod service;

use crate::discord::interaction::{Ix, Reply};
use anyhow::Result;
use twilight_model::{
    guild::Permissions,
    id::{Id, marker::GuildMarker},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Tous,
    Membre,
    Grade,
    Lead,
    Admin,
}

impl Level {
    pub const ALL: [Self; 5] = [Self::Tous, Self::Membre, Self::Grade, Self::Lead, Self::Admin];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Tous => "tous",
            Self::Membre => "membre",
            Self::Grade => "grade",
            Self::Lead => "lead",
            Self::Admin => "admin",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|l| l.as_str() == raw.trim())
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Tous => "Tout le monde",
            Self::Membre => "Membres du groupe",
            Self::Grade => "Gradés",
            Self::Lead => "Leads",
            Self::Admin => "« Gérer le serveur »",
        }
    }

    /// Qui passe, pour les messages de refus.
    pub fn who(self) -> &'static str {
        match self {
            Self::Tous => "tout le monde",
            Self::Membre => "aux membres du groupe, gradés et leads",
            Self::Grade => "aux gradés et leads",
            Self::Lead => "aux leads",
            Self::Admin => "à « Gérer le serveur »",
        }
    }
}

macro_rules! perms {
    ($($variant:ident => $key:literal, $module:literal, $label:literal, $default:ident, $strict:literal;)*) => {
        /// Action soumise à permission.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum Perm {
            $($variant,)*
        }

        impl Perm {
            pub const ALL: &'static [Self] = &[$(Self::$variant,)*];

            pub fn key(self) -> &'static str {
                match self { $(Self::$variant => $key,)* }
            }

            /// Module, pour regrouper l'affichage.
            pub fn module(self) -> &'static str {
                match self { $(Self::$variant => $module,)* }
            }

            pub fn label(self) -> &'static str {
                match self { $(Self::$variant => $label,)* }
            }

            pub fn default_level(self) -> Level {
                match self { $(Self::$variant => Level::$default,)* }
            }

            /// Strict : seuls les rôles comptent, pas les permissions Discord.
            pub fn strict(self) -> bool {
                match self { $(Self::$variant => $strict,)* }
            }
        }
    };
}

perms! {
    PresenceCreer => "presence.creer", "Présences", "Créer, relancer et clôturer une présence", Tous, false;
    PresenceStats => "presence.stats", "Présences", "Statistiques et historique d'une présence", Tous, false;
    PresenceGerer => "presence.gerer", "Présences", "Forcer une réponse, annuler une présence, planning", Grade, false;
    PresenceReset => "presence.reset", "Présences", "Réinitialiser les statistiques de présence", Admin, false;
    Blocklist => "presence.blocklist", "Présences", "Ajouter ou retirer un membre de la blocklist", Grade, false;
    AbsenceAutrui => "absence.autrui", "Absences", "Absence d'un autre membre, panneau des absences", Grade, false;
    TransactionUtiliser => "transaction.utiliser", "Transactions", "Créer, compléter, changer le statut, annuler", Tous, false;
    TransactionSupprimer => "transaction.supprimer", "Transactions", "Supprimer définitivement une transaction", Grade, false;
    TransactionHistorique => "transaction.historique", "Transactions", "Historique et export sur le site", Admin, false;
    Vehicule => "vehicule.utiliser", "Véhicules", "Enregistrer, consulter, modifier, retirer un véhicule", Tous, false;
    NumeroAjouterAutrui => "numero.ajouter_autrui", "Numéros", "Enregistrer le numéro d'un autre membre", Admin, false;
    NumeroSupprimerAutrui => "numero.supprimer_autrui", "Numéros", "Supprimer le numéro d'un autre membre", Grade, false;
    RecrueSuivi => "recrue.suivi", "Recrues", "Liste, détail et historique des avis, ajout et retrait, salon d'essai", Grade, true;
    RecrueAvis => "recrue.avis", "Recrues", "Donner un avis sur une recrue", Grade, true;
    RecrueVeto => "recrue.veto", "Recrues", "Poser ou lever un véto", Lead, true;
    RecrueReset => "recrue.reset", "Recrues", "Forcer une nouvelle semaine d'avis", Admin, false;
    CotisationAjouter => "cotisation.ajouter", "Cotisations", "Déclarer ses cotisations", Tous, false;
    CotisationGerer => "cotisation.gerer", "Cotisations", "Cotisations des autres : déclarer, consulter, supprimer", Grade, false;
    CotisationStats => "cotisation.stats", "Cotisations", "Statistiques (Discord et site)", Grade, false;
    CoffreVoir => "coffre.voir", "Coffres", "Stock et historique (Discord et site)", Grade, false;
    CoffreGerer => "coffre.gerer", "Coffres", "Relier un personnage, nommer une propriété", Grade, false;
    ContactVoir => "contact.voir", "Contacts", "Consulter le répertoire", Tous, false;
    ContactAjouter => "contact.ajouter", "Contacts", "Ajouter un contact", Membre, false;
    ContactModifier => "contact.modifier", "Contacts", "Modifier ou supprimer un contact", Grade, false;
    ContactNotes => "contact.notes", "Contacts", "Lire et écrire les notes du staff (Discord et site)", Grade, false;
    SiteStats => "site.stats", "Site", "Statistiques du serveur sur le site", Admin, false;
    Journal => "site.journal", "Site", "Journal du staff sur le site", Admin, false;
}

impl Perm {
    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|p| p.key() == raw.trim())
    }
}

/// Ce qu'une personne détient sur un serveur.
#[derive(Clone, Debug, Default)]
pub struct Holder {
    pub roles: Vec<String>,
    pub manage_guild: bool,
    pub manage_messages: bool,
}

/// Rôles des niveaux sur un serveur.
#[derive(Clone, Debug, Default)]
pub struct Roles {
    pub member: Option<String>,
    pub grade: Option<String>,
    pub lead: Option<String>,
}

impl Roles {
    /// Niveau le plus haut atteint par une personne.
    pub fn level_of(&self, holder: &Holder, strict: bool) -> Option<Level> {
        let has = |role: &Option<String>| role.as_ref().is_some_and(|r| holder.roles.contains(r));
        if !strict && holder.manage_guild {
            Some(Level::Admin)
        } else if has(&self.lead) {
            Some(Level::Lead)
        } else if has(&self.grade) || (!strict && holder.manage_messages) {
            Some(Level::Grade)
        } else if has(&self.member) {
            Some(Level::Membre)
        } else {
            None
        }
    }

    /// `level` requis pour l'action : la personne a-t-elle le droit ?
    pub fn allows(&self, holder: &Holder, level: Level, strict: bool) -> bool {
        level == Level::Tous || self.level_of(holder, strict).is_some_and(|held| held >= level)
    }
}

/// Droits d'une personne sur un serveur, chargés une fois (Discord ou site).
pub struct Grants {
    roles: Roles,
    levels: std::collections::HashMap<String, Level>,
    holder: Holder,
}

impl Grants {
    pub async fn load(db: &sqlx::SqlitePool, guild_id: Id<GuildMarker>, holder: Holder) -> Result<Self> {
        let guild = guild_id.to_string();
        Ok(Self { roles: service::roles(db, &guild).await?, levels: service::levels(db, &guild).await?, holder })
    }

    pub fn level(&self, perm: Perm) -> Level {
        self.levels.get(perm.key()).copied().unwrap_or(perm.default_level())
    }

    pub fn can(&self, perm: Perm) -> bool {
        self.roles.allows(&self.holder, self.level(perm), perm.strict())
    }
}

fn holder(ix: &Ix) -> Holder {
    Holder {
        roles: ix.member_roles().iter().map(ToString::to_string).collect(),
        manage_guild: ix.has_permission(Permissions::MANAGE_GUILD),
        manage_messages: ix.has_permission(Permissions::MANAGE_MESSAGES),
    }
}

/// La personne à l'origine de l'interaction peut-elle faire `perm` ? Hors
/// serveur : seulement les actions ouvertes à tous par défaut.
pub async fn can(ix: &Ix, perm: Perm) -> Result<bool> {
    let Some(guild_id) = ix.guild_id() else {
        return Ok(perm.default_level() == Level::Tous);
    };
    Ok(Grants::load(&ix.ctx.db, guild_id, holder(ix)).await?.can(perm))
}

/// Comme [`can`], et répond le refus à la personne. Usage :
/// `if !permissions::require(ix, Perm::X).await? { return Ok(()); }`
pub async fn require(ix: &Ix, perm: Perm) -> Result<bool> {
    let Some(guild_id) = ix.guild_id() else {
        return can(ix, perm).await;
    };
    let grants = Grants::load(&ix.ctx.db, guild_id, holder(ix)).await?;
    if grants.can(perm) {
        return Ok(true);
    }
    ix.reply(Reply::ephemeral(denied(perm, grants.level(perm)))).await?;
    Ok(false)
}

pub fn denied(perm: Perm, level: Level) -> String {
    format!("« {} » est réservé {}.", perm.label(), level.who())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn roles() -> Roles {
        Roles { member: Some("1".into()), grade: Some("2".into()), lead: Some("3".into()) }
    }

    fn with(roles: &[&str]) -> Holder {
        Holder { roles: roles.iter().map(|r| (*r).to_owned()).collect(), ..Default::default() }
    }

    #[test]
    fn levels_nest() {
        let r = roles();
        assert!(r.allows(&with(&[]), Level::Tous, false));
        assert!(!r.allows(&with(&[]), Level::Membre, false));
        assert!(r.allows(&with(&["1"]), Level::Membre, false));
        assert!(!r.allows(&with(&["1"]), Level::Grade, false));
        assert!(r.allows(&with(&["3"]), Level::Membre, false));
        assert!(r.allows(&with(&["3"]), Level::Lead, false));
        assert!(!r.allows(&with(&["3"]), Level::Admin, false));
    }

    #[test]
    fn discord_permissions_count_unless_strict() {
        let r = roles();
        let admin = Holder { manage_guild: true, ..Default::default() };
        let moderator = Holder { manage_messages: true, ..Default::default() };
        assert!(r.allows(&admin, Level::Admin, false));
        assert!(r.allows(&admin, Level::Lead, false));
        assert!(!r.allows(&admin, Level::Grade, true));
        assert!(r.allows(&moderator, Level::Grade, false));
        assert!(!r.allows(&moderator, Level::Lead, false));
        assert!(!r.allows(&moderator, Level::Grade, true));
    }

    #[test]
    fn unconfigured_member_role_lets_nobody_through() {
        let r = Roles { member: None, ..roles() };
        assert!(!r.allows(&with(&["1"]), Level::Membre, false));
        assert!(r.allows(&with(&["2"]), Level::Membre, false));
    }

    #[test]
    fn keys_are_unique_and_parse_back() {
        for perm in Perm::ALL {
            assert_eq!(Perm::parse(perm.key()), Some(*perm));
        }
        let mut keys: Vec<_> = Perm::ALL.iter().map(|p| p.key()).collect();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(keys.len(), Perm::ALL.len());
        for level in Level::ALL {
            assert_eq!(Level::parse(level.as_str()), Some(level));
        }
    }
}
