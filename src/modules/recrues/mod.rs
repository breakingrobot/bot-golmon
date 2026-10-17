//! Recrues : avis hebdomadaires des gradés sur les membres en période d'essai,
//! avec droit de véto des leads. (Anciennement « prospects ».)

pub mod channels;
pub mod commands;
pub mod cron;
pub mod embed;
pub mod events;
pub mod interactions;
pub mod reminder;
pub mod service;

use crate::discord::interaction::Ix;
use service::RecrueConfig;

pub const NAMESPACE: &str = "recrue";
pub const COLOR: u32 = 0x5865f2;
pub const VETO_COLOR: u32 = 0xed4245;
/// Longueur des motifs dans le tableau de bord ; le détail complet est ailleurs.
pub const MOTIF_PREVIEW: usize = 90;
pub const MOTIF_MAX_LENGTH: u16 = 500;
/// Discord plafonne un menu déroulant à 25 options.
pub const SELECT_MAX_OPTIONS: usize = 25;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Vote {
    Pour,
    Contre,
    Blanc,
}

impl Vote {
    /// Ordre d'affichage stable.
    pub const ORDER: [Self; 3] = [Self::Pour, Self::Contre, Self::Blanc];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pour => "POUR",
            Self::Contre => "CONTRE",
            Self::Blanc => "BLANC",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Self::ORDER.into_iter().find(|v| v.as_str() == value)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pour => "Pour",
            Self::Contre => "Contre",
            Self::Blanc => "Blanc",
        }
    }

    pub fn emoji(self) -> &'static str {
        match self {
            Self::Pour => "✅",
            Self::Contre => "❌",
            Self::Blanc => "⬜",
        }
    }
}

pub const NOT_GRADE: &str = "Seuls les membres ayant le rôle gradé (ou lead) peuvent donner un avis sur les recrues.";
pub const NOT_LEAD: &str = "Seuls les membres ayant le rôle lead peuvent poser ou lever un véto.";

/// Contrôle d'accès strictement basé sur les rôles configurés. Volontairement,
/// « Gérer le serveur » n'ouvre aucun droit ici : un administrateur n'est pas
/// un lead. Cette permission ne sert qu'à `/recrue config` et `/recrue reset`.
fn has_role(ix: &Ix, role_id: Option<&String>) -> bool {
    role_id.is_some_and(|role| ix.member_roles().iter().any(|r| r.to_string() == *role))
}

/// Le véto est réservé au rôle lead configuré.
pub fn is_lead(ix: &Ix, config: Option<&RecrueConfig>) -> bool {
    has_role(ix, config.and_then(|c| c.lead_role_id.as_ref()))
}

/// Un lead peut tout ce qu'un gradé peut faire, et dispose en plus du véto.
pub fn is_grade(ix: &Ix, config: Option<&RecrueConfig>) -> bool {
    has_role(ix, config.and_then(|c| c.grade_role_id.as_ref())) || is_lead(ix, config)
}
