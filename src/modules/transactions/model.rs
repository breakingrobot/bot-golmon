use crate::util::time::DbTime;
use sqlx::FromRow;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Status {
    Preparation,
    EnAttenteRdv,
    AvanceAttentePaiement,
    LivrePaye,
    Annulee,
    Cloturee,
}

impl Status {
    pub const ALL: [Self; 6] = [
        Self::Preparation,
        Self::EnAttenteRdv,
        Self::AvanceAttentePaiement,
        Self::LivrePaye,
        Self::Annulee,
        Self::Cloturee,
    ];
    /// Statuts en cours, listés intégralement dans le tableau de bord.
    pub const ACTIVE: [Self; 3] = [Self::Preparation, Self::EnAttenteRdv, Self::AvanceAttentePaiement];
    /// Statuts terminaux : seuls les plus récents sont affichés.
    pub const TERMINAL: [Self; 3] = [Self::LivrePaye, Self::Annulee, Self::Cloturee];
    /// Proposés par `/transaction status`. `ANNULEE` en est exclu : c'est le rôle
    /// de `/transaction cancel`, et le proposer aux deux endroits invite à annuler
    /// une transaction en croyant la faire avancer.
    pub const CHOICES: [Self; 5] =
        [Self::Preparation, Self::EnAttenteRdv, Self::AvanceAttentePaiement, Self::LivrePaye, Self::Cloturee];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Preparation => "PREPARATION",
            Self::EnAttenteRdv => "EN_ATTENTE_RDV",
            Self::AvanceAttentePaiement => "AVANCE_ATTENTE_PAIEMENT",
            Self::LivrePaye => "LIVRE_PAYE",
            Self::Annulee => "ANNULEE",
            Self::Cloturee => "CLOTUREE",
        }
    }

    /// Le `custom_id` d'un bouton est fourni par le client : rien n'en garantit le contenu.
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == value)
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Preparation => "Préparation",
            Self::EnAttenteRdv => "En attente RDV",
            Self::AvanceAttentePaiement => "Avance (en attente de paiement)",
            Self::LivrePaye => "Livré et payé",
            Self::Annulee => "Annulée",
            Self::Cloturee => "Clôturée",
        }
    }

    pub fn emoji(self) -> &'static str {
        match self {
            Self::Preparation => "🔧",
            Self::EnAttenteRdv => "📅",
            Self::AvanceAttentePaiement => "💰",
            Self::LivrePaye => "✅",
            Self::Annulee => "❌",
            Self::Cloturee => "🔒",
        }
    }

    pub fn color(self) -> u32 {
        match self {
            Self::Preparation => 0x95a5a6,
            Self::EnAttenteRdv => 0xf1c40f,
            Self::AvanceAttentePaiement => 0xe67e22,
            Self::LivrePaye => 0x2ecc71,
            Self::Annulee => 0xe74c3c,
            Self::Cloturee => 0x7f8c8d,
        }
    }
}

pub fn type_label(kind: &str) -> &'static str {
    match kind {
        "ACHAT" => "Achat",
        _ => "Vente",
    }
}

pub const TYPE_CHOICES: [(&str, &str); 2] = [("Achat", "ACHAT"), ("Vente", "VENTE")];

/// Note d'une transaction : 500 caractères au plus.
pub const NOTE_LIMIT: usize = 500;

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct TransactionConfig {
    pub dashboard_channel_id: Option<String>,
    pub dashboard_message_id: Option<String>,
    pub ping_role_id: Option<String>,
}

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct Transaction {
    pub id: String,
    pub channel_id: String,
    pub message_id: Option<String>,
    pub groupe: String,
    #[sqlx(rename = "type")]
    pub kind: String,
    pub prix: f64,
    pub statut: String,
    pub created_by_id: String,
    pub updated_at: DbTime,
    /// Note libre, affichée dans la fiche.
    pub note: Option<String>,
    #[sqlx(skip)]
    pub items: Vec<TransactionItem>,
}

impl Transaction {
    pub fn status(&self) -> Status {
        Status::parse(&self.statut).unwrap_or(Status::Preparation)
    }
}

#[derive(Clone, Debug, FromRow)]
#[sqlx(rename_all = "camelCase")]
pub struct TransactionItem {
    pub id: String,
    pub transaction_id: String,
    pub nom: String,
    pub quantite: i64,
    pub prix_unitaire: Option<f64>,
}
