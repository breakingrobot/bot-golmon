//! Constructeurs de composants (boutons, menus, champs de modal).

use twilight_model::channel::message::{
    Component, EmojiReactionType,
    component::{
        ActionRow, Button, ButtonStyle, Label, SelectMenu, SelectMenuOption, SelectMenuType, TextInput, TextInputStyle,
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
