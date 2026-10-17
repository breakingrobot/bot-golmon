//! Lecture des options d'une slash command, sous-commandes aplaties.

use twilight_model::{
    application::interaction::{
        InteractionDataResolved,
        application_command::{CommandData, CommandDataOption, CommandOptionValue},
    },
    id::{
        Id,
        marker::{ChannelMarker, GenericMarker, RoleMarker, UserMarker},
    },
};

pub struct Options {
    /// Groupe et sous-commande, ex : `["blocklist", "add"]` ou `["create"]`.
    pub path: Vec<String>,
    values: Vec<(String, CommandOptionValue)>,
    /// Option en cours de saisie (autocomplétion).
    pub focused: Option<(String, String)>,
    resolved: Option<InteractionDataResolved>,
}

/// Cible d'une option « mentionnable » : membre ou rôle.
pub enum Mentionable {
    User(Id<UserMarker>),
    Role(Id<RoleMarker>),
}

impl Options {
    pub fn parse(data: &CommandData) -> Self {
        let mut options = Self { path: Vec::new(), values: Vec::new(), focused: None, resolved: data.resolved.clone() };
        options.walk(&data.options);
        options
    }

    fn walk(&mut self, options: &[CommandDataOption]) {
        for option in options {
            match &option.value {
                CommandOptionValue::SubCommand(inner) | CommandOptionValue::SubCommandGroup(inner) => {
                    self.path.push(option.name.clone());
                    self.walk(inner);
                }
                CommandOptionValue::Focused(value, _) => {
                    self.focused = Some((option.name.clone(), value.clone()));
                }
                value => self.values.push((option.name.clone(), value.clone())),
            }
        }
    }

    pub fn group(&self) -> Option<&str> {
        (self.path.len() > 1).then(|| self.path[0].as_str())
    }

    pub fn sub(&self) -> &str {
        self.path.last().map(String::as_str).unwrap_or_default()
    }

    fn get(&self, name: &str) -> Option<&CommandOptionValue> {
        self.values.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    pub fn string(&self, name: &str) -> Option<String> {
        match self.get(name)? {
            CommandOptionValue::String(value) => Some(value.clone()),
            _ => None,
        }
    }

    pub fn integer(&self, name: &str) -> Option<i64> {
        match self.get(name)? {
            CommandOptionValue::Integer(value) => Some(*value),
            _ => None,
        }
    }

    pub fn number(&self, name: &str) -> Option<f64> {
        match self.get(name)? {
            CommandOptionValue::Number(value) => Some(*value),
            CommandOptionValue::Integer(value) => Some(*value as f64),
            _ => None,
        }
    }

    pub fn boolean(&self, name: &str) -> Option<bool> {
        match self.get(name)? {
            CommandOptionValue::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    pub fn user(&self, name: &str) -> Option<Id<UserMarker>> {
        match self.get(name)? {
            CommandOptionValue::User(value) => Some(*value),
            _ => None,
        }
    }

    /// Nom d'utilisateur d'une option `user`, résolu par Discord.
    pub fn user_name(&self, id: Id<UserMarker>) -> String {
        self.resolved.as_ref().and_then(|r| r.users.get(&id)).map(|u| u.name.clone()).unwrap_or_else(|| id.to_string())
    }

    /// Utilisateur complet d'une option `user` (avatar, nom…), résolu par Discord.
    pub fn resolved_user(&self, id: Id<UserMarker>) -> Option<twilight_model::user::User> {
        self.resolved.as_ref().and_then(|r| r.users.get(&id)).cloned()
    }

    pub fn role(&self, name: &str) -> Option<Id<RoleMarker>> {
        match self.get(name)? {
            CommandOptionValue::Role(value) => Some(*value),
            _ => None,
        }
    }

    pub fn channel(&self, name: &str) -> Option<Id<ChannelMarker>> {
        match self.get(name)? {
            CommandOptionValue::Channel(value) => Some(*value),
            _ => None,
        }
    }

    /// Options saisies, lisibles (« salon : #… »), pour le journal du staff.
    pub fn summary(&self) -> String {
        self.values
            .iter()
            .map(|(name, value)| {
                let shown = match value {
                    CommandOptionValue::String(v) => v.clone(),
                    CommandOptionValue::Integer(v) => v.to_string(),
                    CommandOptionValue::Number(v) => v.to_string(),
                    CommandOptionValue::Boolean(v) => if *v { "oui" } else { "non" }.to_owned(),
                    CommandOptionValue::User(id) => format!("<@{id}>"),
                    CommandOptionValue::Role(id) => format!("<@&{id}>"),
                    CommandOptionValue::Channel(id) => format!("<#{id}>"),
                    CommandOptionValue::Mentionable(id) => format!("<@{id}>"),
                    _ => return String::new(),
                };
                format!("{name} : {shown}")
            })
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Discord indique dans `resolved` si l'identifiant mentionné est un rôle.
    pub fn mentionable(&self, name: &str) -> Option<Mentionable> {
        let id: Id<GenericMarker> = match self.get(name)? {
            CommandOptionValue::Mentionable(value) => *value,
            CommandOptionValue::User(value) => return Some(Mentionable::User(*value)),
            CommandOptionValue::Role(value) => return Some(Mentionable::Role(*value)),
            _ => return None,
        };
        let is_role = self.resolved.as_ref().is_some_and(|r| r.roles.contains_key(&id.cast()));
        Some(if is_role { Mentionable::Role(id.cast()) } else { Mentionable::User(id.cast()) })
    }
}
