//! Répertoire des contacts : autres groupes, entreprises et personnes du jeu,
//! avec leur relation au groupe (allié, neutre, hostile). Tout le monde consulte
//! le répertoire ; les membres du groupe y ajoutent des contacts, les gradés et
//! leads le tiennent à jour et y ajoutent des notes réservées au staff. Niveaux
//! réglables avec `/config permission`.

pub mod commands;
pub mod service;

pub const NAME_LIMIT: usize = 80;
pub const PHONE_LIMIT: usize = 30;
pub const DESCRIPTION_LIMIT: usize = 500;
pub const NOTE_LIMIT: usize = 1000;

pub const NAME_TAKEN: &str = "Un contact porte déjà ce nom (sans tenir compte des majuscules).";

/// Nature du contact.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Groupe,
    Entreprise,
    Personne,
}

impl Kind {
    pub const ALL: [Self; 3] = [Self::Groupe, Self::Entreprise, Self::Personne];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Groupe => "GROUPE",
            Self::Entreprise => "ENTREPRISE",
            Self::Personne => "PERSONNE",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|k| k.as_str().eq_ignore_ascii_case(raw.trim()))
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Groupe => "Groupe",
            Self::Entreprise => "Entreprise",
            Self::Personne => "Personne",
        }
    }

    pub fn emoji(self) -> &'static str {
        match self {
            Self::Groupe => "👥",
            Self::Entreprise => "🏢",
            Self::Personne => "👤",
        }
    }
}

/// Relation du contact avec le groupe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relation {
    Allie,
    Neutre,
    Hostile,
}

impl Relation {
    /// Ordre d'affichage : alliés, neutres, hostiles.
    pub const ALL: [Self; 3] = [Self::Allie, Self::Neutre, Self::Hostile];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Allie => "ALLIE",
            Self::Neutre => "NEUTRE",
            Self::Hostile => "HOSTILE",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.as_str().eq_ignore_ascii_case(raw.trim()))
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Allie => "Allié",
            Self::Neutre => "Neutre",
            Self::Hostile => "Hostile",
        }
    }

    /// Intitulé d'un groupe de contacts.
    pub fn plural_label(self) -> &'static str {
        match self {
            Self::Allie => "Alliés",
            Self::Neutre => "Neutres",
            Self::Hostile => "Hostiles",
        }
    }

    pub fn emoji(self) -> &'static str {
        match self {
            Self::Allie => "🟢",
            Self::Neutre => "⚪",
            Self::Hostile => "🔴",
        }
    }

    /// « 🟢 Allié ».
    pub fn display(self) -> String {
        format!("{} {}", self.emoji(), self.label())
    }

    pub fn color(self) -> u32 {
        match self {
            Self::Allie => 0x2ecc71,
            Self::Neutre => 0x95a5a6,
            Self::Hostile => 0xe74c3c,
        }
    }
}

/// Texte libre facultatif : espaces retirés ; vide ou « - » = absent.
pub fn optional_text(raw: Option<String>) -> Option<String> {
    raw.map(|text| text.trim().to_owned()).filter(|text| !text.is_empty() && text != "-")
}

/// Vérifie les champs d'un contact ; `Err` porte le message pour l'utilisateur.
pub fn validate(name: &str, phone: Option<&str>, description: Option<&str>) -> Result<(), String> {
    let count = |text: &str| text.chars().count();
    if name.trim().is_empty() {
        return Err("Le nom du contact est vide.".to_owned());
    }
    if count(name) > NAME_LIMIT {
        return Err(format!("Le nom ne doit pas dépasser {NAME_LIMIT} caractères."));
    }
    if phone.is_some_and(|p| count(p) > PHONE_LIMIT) {
        return Err(format!("Le téléphone ne doit pas dépasser {PHONE_LIMIT} caractères."));
    }
    if description.is_some_and(|d| count(d) > DESCRIPTION_LIMIT) {
        return Err(format!("La description ne doit pas dépasser {DESCRIPTION_LIMIT} caractères."));
    }
    Ok(())
}

pub fn validate_note(body: &str) -> Result<(), String> {
    if body.trim().is_empty() {
        return Err("La note est vide.".to_owned());
    }
    if body.chars().count() > NOTE_LIMIT {
        return Err(format!("La note ne doit pas dépasser {NOTE_LIMIT} caractères."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_validates() {
        assert_eq!(Relation::parse("allie"), Some(Relation::Allie));
        assert_eq!(Kind::parse("ENTREPRISE"), Some(Kind::Entreprise));
        assert_eq!(Relation::parse("ami"), None);
        assert_eq!(Relation::Hostile.display(), "🔴 Hostile");
        assert_eq!(optional_text(Some("  - ".into())), None);
        assert_eq!(optional_text(Some(" 555-1234 ".into())).as_deref(), Some("555-1234"));

        assert!(validate("Les Ballas", Some("555-1234"), Some("Gang du sud")).is_ok());
        assert!(validate(" ", None, None).is_err());
        assert!(validate(&"é".repeat(80), None, None).is_ok());
        assert!(validate(&"é".repeat(81), None, None).is_err());
        assert!(validate("x", None, Some(&"a".repeat(501))).is_err());
        assert!(validate_note(&"a".repeat(1000)).is_ok());
        assert!(validate_note(&"a".repeat(1001)).is_err());
        assert!(validate_note("  ").is_err());
    }
}
