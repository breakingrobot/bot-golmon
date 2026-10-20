//! Liens vers les pages du site (SPA) : bouton « Ouvrir sur le site » sous les
//! réponses et tableaux de bord. Sans site configuré (`WEB_URL`), aucun lien.
//!
//! Les adresses suivent les routes de l'interface (`ui/src/routes`) et leurs
//! paramètres de recherche (`ui/src/pages/*/…-search.ts`).

use crate::{
    context::Ctx,
    discord::components::{ButtonExt as _, row},
};
use twilight_model::{
    channel::message::{
        Component,
        component::{Button, ButtonStyle},
    },
    id::{
        Id,
        marker::{GuildMarker, UserMarker},
    },
};

pub const LABEL: &str = "Ouvrir sur le site";

/// Page du site d'un serveur, avec ses filtres éventuels. Toutes les pages sont
/// listées, même celles qu'aucun bouton n'ouvre encore.
#[allow(dead_code)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page<'a> {
    Accueil,
    Transactions { groupe: Option<&'a str> },
    Cotisations { membre: Option<Id<UserMarker>> },
    Coffres { propriete: Option<&'a str>, objet: Option<&'a str> },
    /// `contact` : identifiant d'un contact, ouvre sa fiche.
    Contacts { contact: Option<&'a str> },
    Membres,
    Membre(Id<UserMarker>),
    Journal,
    Permissions,
    Configuration,
    Presences,
    Absences,
    Recrues,
    Annuaire,
}

/// Encode une valeur de paramètre (tout sauf les caractères non réservés).
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

fn query(params: &[(&str, Option<String>)]) -> String {
    let parts: Vec<String> = params
        .iter()
        .filter_map(|(name, value)| value.as_ref().filter(|v| !v.is_empty()).map(|v| format!("{name}={}", encode(v))))
        .collect();
    if parts.is_empty() { String::new() } else { format!("?{}", parts.join("&")) }
}

/// Chemin de la page (sans l'adresse du site).
pub fn path(guild_id: Id<GuildMarker>, page: Page) -> String {
    let base = format!("/serveurs/{guild_id}");
    let owned = |v: Option<&str>| v.map(str::to_owned);
    match page {
        Page::Accueil => base,
        Page::Transactions { groupe } => format!("{base}/transactions{}", query(&[("groupe", owned(groupe))])),
        Page::Cotisations { membre } => {
            format!("{base}/cotisations{}", query(&[("membre", membre.map(|m| m.to_string()))]))
        }
        Page::Coffres { propriete, objet } => {
            format!("{base}/coffres{}", query(&[("propriete", owned(propriete)), ("objet", owned(objet))]))
        }
        Page::Contacts { contact } => format!("{base}/contacts{}", query(&[("contact", owned(contact))])),
        Page::Membres => format!("{base}/membres"),
        Page::Membre(user_id) => format!("{base}/membres/{user_id}"),
        Page::Journal => format!("{base}/journal"),
        Page::Permissions => format!("{base}/permissions"),
        Page::Configuration => format!("{base}/configuration"),
        Page::Presences => format!("{base}/presences"),
        Page::Absences => format!("{base}/absences"),
        Page::Recrues => format!("{base}/recrues"),
        Page::Annuaire => format!("{base}/annuaire"),
    }
}

/// Adresse complète d'une page, `None` si le site n'est pas configuré.
pub fn url(ctx: &Ctx, guild_id: Id<GuildMarker>, page: Page) -> Option<String> {
    crate::web::auth::public_url(ctx).map(|base| format!("{base}{}", path(guild_id, page)))
}

/// Bouton lien 🌐 vers une adresse.
pub fn link_button(url: String, label: &str) -> Component {
    Component::Button(
        Button {
            id: None,
            custom_id: None,
            disabled: false,
            emoji: None,
            label: Some(label.to_owned()),
            style: ButtonStyle::Link,
            url: Some(url),
            sku_id: None,
        }
        .with_emoji("🌐"),
    )
}

/// Bouton « Ouvrir sur le site », `None` sans site configuré.
pub fn button(ctx: &Ctx, guild_id: Id<GuildMarker>, page: Page) -> Option<Component> {
    url(ctx, guild_id, page).map(|url| link_button(url, LABEL))
}

/// Ligne d'un seul bouton « Ouvrir sur le site » (vide sans site configuré).
pub fn row_for(ctx: &Ctx, guild_id: Id<GuildMarker>, page: Page) -> Vec<Component> {
    button(ctx, guild_id, page).map(|b| row(vec![b])).into_iter().collect()
}

/// Ajoute un bouton en bas d'un message Components V2, dans son encadré (ou
/// à la suite s'il n'en a pas). Rien sans bouton.
pub fn attach(components: &mut Vec<Component>, button: Option<Component>) {
    let Some(button) = button else {
        return;
    };
    let link = row(vec![button]);
    match components.last_mut() {
        Some(Component::Container(container)) => container.components.push(link),
        _ => components.push(link),
    }
}

/// Tableau de bord permanent : ajoute « Ouvrir sur le site » si le site est configuré.
pub fn attach_page(ctx: &Ctx, guild_id: &str, page: Page, components: &mut Vec<Component>) {
    let Some(guild_id) = crate::discord::messages::parse_id(guild_id) else {
        return;
    };
    attach(components, button(ctx, guild_id, page));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_spa_paths_with_filters() {
        let guild = Id::new(42);
        assert_eq!(path(guild, Page::Accueil), "/serveurs/42");
        assert_eq!(path(guild, Page::Membre(Id::new(7))), "/serveurs/42/membres/7");
        assert_eq!(path(guild, Page::Coffres { propriete: None, objet: None }), "/serveurs/42/coffres");
        assert_eq!(
            path(guild, Page::Coffres { propriete: Some("8740"), objet: Some("repair_kit") }),
            "/serveurs/42/coffres?propriete=8740&objet=repair_kit"
        );
        assert_eq!(path(guild, Page::Cotisations { membre: Some(Id::new(9)) }), "/serveurs/42/cotisations?membre=9");
        assert_eq!(path(guild, Page::Contacts { contact: Some("c1") }), "/serveurs/42/contacts?contact=c1");
        assert_eq!(
            path(guild, Page::Transactions { groupe: Some("Ballas & Co") }),
            "/serveurs/42/transactions?groupe=Ballas%20%26%20Co"
        );
        assert_eq!(path(guild, Page::Transactions { groupe: Some("") }), "/serveurs/42/transactions");
        assert_eq!(path(guild, Page::Annuaire), "/serveurs/42/annuaire");
    }

    #[test]
    fn encodes_non_ascii() {
        assert_eq!(encode("Benny's é"), "Benny%27s%20%C3%A9");
    }

    #[test]
    fn attaches_inside_the_container() {
        use crate::discord::components::{container, text_display};
        let mut components = vec![container(None, vec![text_display("x")])];
        attach(&mut components, None);
        attach(&mut components, Some(link_button("https://bot.example".into(), LABEL)));
        let Component::Container(inner) = &components[0] else { panic!("encadré attendu") };
        assert!(matches!(inner.components.as_slice(), [Component::TextDisplay(_), Component::ActionRow(_)]));
        let mut bare = Vec::new();
        attach(&mut bare, Some(link_button("https://bot.example".into(), LABEL)));
        assert!(matches!(bare.as_slice(), [Component::ActionRow(_)]));
    }

    #[test]
    fn link_button_has_no_custom_id() {
        let Component::Button(button) = link_button("https://bot.example/x".into(), LABEL) else {
            panic!("bouton attendu");
        };
        assert_eq!((button.style, button.custom_id, button.url.as_deref()), (
            ButtonStyle::Link,
            None,
            Some("https://bot.example/x")
        ));
    }
}
