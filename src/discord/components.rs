//! Constructeurs de composants (boutons, menus, champs de modal, mise en page
//! « Components V2 »).

use crate::util::format::TEXT_DISPLAY_LIMIT;
use twilight_model::channel::message::{
    Component, EmojiReactionType,
    component::{
        ActionRow, Button, ButtonStyle, Container, Label, Section, SelectMenu, SelectMenuOption, SelectMenuType,
        Separator, SeparatorSpacingSize, TextDisplay, TextInput, TextInputStyle,
    },
};

pub fn row(components: Vec<Component>) -> Component {
    Component::ActionRow(ActionRow { id: None, components })
}

pub fn emoji(unicode: &str) -> EmojiReactionType {
    EmojiReactionType::Unicode { name: unicode.to_owned() }
}

pub fn button(custom_id: String, label: impl Into<String>, style: ButtonStyle) -> Button {
    Button {
        id: None,
        custom_id: Some(custom_id),
        disabled: false,
        emoji: None,
        label: Some(label.into()),
        style,
        url: None,
        sku_id: None,
    }
}

pub trait ButtonExt {
    fn with_emoji(self, unicode: &str) -> Self;
    fn disabled(self, disabled: bool) -> Self;
}

impl ButtonExt for Button {
    fn with_emoji(mut self, unicode: &str) -> Self {
        self.emoji = Some(emoji(unicode));
        self
    }

    fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }
}

pub fn select_option(label: impl Into<String>, value: impl Into<String>) -> SelectMenuOption {
    SelectMenuOption { default: false, description: None, emoji: None, label: label.into(), value: value.into() }
}

pub fn string_select(custom_id: String, placeholder: &str, options: Vec<SelectMenuOption>) -> SelectMenu {
    SelectMenu {
        id: None,
        channel_types: None,
        custom_id,
        default_values: None,
        disabled: false,
        kind: SelectMenuType::Text,
        max_values: None,
        min_values: None,
        options: Some(options),
        placeholder: Some(placeholder.to_owned()),
        required: None,
    }
}

#[allow(deprecated)] // `label` est porté par le composant Label englobant.
pub fn text_input(custom_id: &str, style: TextInputStyle) -> TextInput {
    TextInput {
        id: None,
        custom_id: custom_id.to_owned(),
        label: None,
        max_length: None,
        min_length: None,
        placeholder: None,
        required: Some(true),
        style,
        value: None,
    }
}

/// Champ de modal : un libellé (et une aide) au-dessus du composant.
pub fn label(text: &str, description: Option<&str>, component: Component) -> Component {
    Component::Label(Label {
        id: None,
        label: text.to_owned(),
        description: description.map(str::to_owned),
        component: Box::new(component),
    })
}

/// Texte Markdown (Components V2).
pub fn text_display(content: impl Into<String>) -> Component {
    Component::TextDisplay(TextDisplay { id: None, content: content.into() })
}

/// Découpe un texte en TextDisplay d'au plus 2000 octets, aux sauts de ligne.
/// Une ligne plus longue à elle seule est coupée.
pub fn text_displays(text: &str) -> Vec<Component> {
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in text.split('\n') {
        if !current.is_empty() && current.len() + 1 + line.len() > TEXT_DISPLAY_LIMIT {
            chunks.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push('\n');
        }
        for ch in line.chars() {
            if current.len() + ch.len_utf8() > TEXT_DISPLAY_LIMIT {
                chunks.push(std::mem::take(&mut current));
            }
            current.push(ch);
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks.into_iter().map(text_display).collect()
}

/// Ligne de séparation entre deux sections.
pub fn separator() -> Component {
    Component::Separator(Separator { id: None, divider: Some(true), spacing: Some(SeparatorSpacingSize::Small) })
}

/// Encadré regroupant un message V2, avec une barre de couleur optionnelle.
pub fn container(accent_color: Option<u32>, components: Vec<Component>) -> Component {
    Component::Container(Container { id: None, accent_color: Some(accent_color), spoiler: None, components })
}

/// Un à trois textes accompagnés d'un accessoire à droite (bouton ou vignette).
pub fn section(texts: Vec<String>, accessory: Component) -> Component {
    Component::Section(Section {
        id: None,
        components: texts.into_iter().map(text_display).collect(),
        accessory: Box::new(accessory),
    })
}

/// Usage des limites V2 pour les tests : (octets de texte, nombre de
/// composants), imbrications et accessoires compris.
#[cfg(test)]
pub fn v2_usage(components: &[Component]) -> (usize, usize) {
    components.iter().fold((0, 0), |(text, count), component| {
        let (inner_text, inner_count) = match component {
            Component::TextDisplay(display) => {
                assert!(display.content.len() <= TEXT_DISPLAY_LIMIT, "TextDisplay trop long");
                (display.content.len(), 0)
            }
            Component::Container(container) => v2_usage(&container.components),
            Component::ActionRow(row) => v2_usage(&row.components),
            Component::Section(section) => {
                let (texts, children) = v2_usage(&section.components);
                let (_, accessory) = v2_usage(std::slice::from_ref(&section.accessory));
                (texts, children + accessory)
            }
            _ => (0, 0),
        };
        (text + inner_text, count + 1 + inner_count)
    })
}

/// Texte de tous les TextDisplay, imbriqués compris, pour les tests.
#[cfg(test)]
pub fn v2_text(components: &[Component]) -> String {
    components
        .iter()
        .map(|component| match component {
            Component::TextDisplay(display) => display.content.clone(),
            Component::Container(container) => v2_text(&container.components),
            Component::Section(section) => v2_text(&section.components),
            _ => String::new(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_text_displays_under_limit() {
        let text = (0..300).map(|i| format!("- ligne n°{i} — éàü")).collect::<Vec<_>>().join("\n");
        let displays = text_displays(&text);
        assert!(displays.len() > 1);
        assert_eq!(v2_text(&displays), text);
        let (bytes, count) = v2_usage(&displays);
        assert_eq!((bytes + displays.len() - 1, count), (text.len(), displays.len()));
        assert_eq!(text_displays(&"é".repeat(1500)).len(), 2);
    }
}
