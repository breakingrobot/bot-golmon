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
