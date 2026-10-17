//! Analyse des commandes texte, au comportement de `knub-command-manager` :
//! plusieurs signatures par commande, essayées dans l'ordre d'enregistrement.

use super::util::{convert_delay_string_to_ms, get_user_mention};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Str,
    Number,
    UserId,
    Delay,
}

#[derive(Clone, Copy, Debug)]
pub enum Param {
    Required(&'static str, Kind),
    Optional(&'static str, Kind),
    /// Reste du message (`text$`).
    Rest(&'static str, bool),
    /// Liste de mots (`opts...`).
    Variadic(&'static str),
}

#[derive(Clone, Debug)]
pub enum Value {
    Str(String),
    Number(i64),
    Delay(i64),
    List(Vec<String>),
}

#[derive(Debug, Default)]
pub struct Args {
    values: HashMap<&'static str, Value>,
    pub switches: Vec<&'static str>,
}

impl Args {
    pub fn str(&self, name: &str) -> Option<&str> {
        match self.values.get(name)? {
            Value::Str(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn number(&self, name: &str) -> Option<i64> {
        match self.values.get(name)? {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn delay(&self, name: &str) -> Option<i64> {
        match self.values.get(name)? {
            Value::Delay(ms) => Some(*ms),
            _ => None,
        }
    }

    pub fn list(&self, name: &str) -> Vec<String> {
        match self.values.get(name) {
            Some(Value::List(items)) => items.clone(),
            _ => Vec::new(),
        }
    }

    pub fn switch(&self, name: &str) -> bool {
        self.switches.contains(&name)
    }
}

/// Mot du message, avec sa position (pour les paramètres « reste du message »).
struct Token {
    value: String,
    start: usize,
}

/// Découpe en mots, en respectant les guillemets simples et doubles.
fn tokenize(input: &str) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();
    while let Some(&(start, ch)) = chars.peek() {
        if ch.is_whitespace() {
            chars.next();
            continue;
        }
        let mut value = String::new();
        if ch == '"' || ch == '\'' {
            chars.next();
            let mut closed = false;
            for (_, c) in chars.by_ref() {
                if c == ch {
                    closed = true;
                    break;
                }
                value.push(c);
            }
            if !closed {
                value.insert(0, ch);
            }
        } else {
            while let Some(&(_, c)) = chars.peek() {
                if c.is_whitespace() {
                    break;
                }
                value.push(c);
                chars.next();
            }
        }
        tokens.push(Token { value, start });
    }
    tokens
}

/// Convertit les arguments selon la signature. `Err` décrit l'argument fautif.
pub fn parse(input: &str, params: &[Param], switches: &[(&'static str, &'static str)]) -> Result<Args, String> {
    let mut args = Args::default();
    let mut tokens: Vec<Token> = tokenize(input);

    if !switches.is_empty() {
        tokens.retain(|token| {
            let name = token.value.strip_prefix("--").or_else(|| token.value.strip_prefix('-'));
            match name.and_then(|n| switches.iter().find(|(long, short)| *long == n || *short == n)) {
                Some((long, _)) => {
                    args.switches.push(long);
                    false
                }
                None => true,
            }
        });
    }

    let mut index = 0;
    for param in params {
        match *param {
            Param::Rest(name, required) => {
                let rest = tokens.get(index).map(|t| input[t.start..].trim().to_owned()).unwrap_or_default();
                if rest.is_empty() {
                    if required {
                        return Err(format!("Missing argument: {name}"));
                    }
                } else {
                    args.values.insert(name, Value::Str(rest));
                }
                index = tokens.len();
            }
            Param::Variadic(name) => {
                args.values.insert(
                    name,
                    Value::List(tokens[index.min(tokens.len())..].iter().map(|t| t.value.clone()).collect()),
                );
                index = tokens.len();
            }
            Param::Required(name, kind) | Param::Optional(name, kind) => {
                let Some(token) = tokens.get(index) else {
                    if matches!(param, Param::Required(..)) {
                        return Err(format!("Missing argument: {name}"));
                    }
                    continue;
                };
                let value = match kind {
                    Kind::Str => Some(Value::Str(token.value.clone())),
                    Kind::Number => {
                        token.value.parse::<f64>().ok().filter(|n| n.is_finite()).map(|n| Value::Number(n as i64))
                    }
                    Kind::UserId => get_user_mention(&token.value).map(Value::Str),
                    Kind::Delay => convert_delay_string_to_ms(&token.value).map(Value::Delay),
                };
                match value {
                    Some(value) => {
                        args.values.insert(name, value);
                        index += 1;
                    }
                    None => return Err(format!("Could not convert argument {name}'s type")),
                }
            }
        }
    }

    if index < tokens.len() {
        return Err("Too many arguments".to_owned());
    }
    Ok(args)
}

/// Découpe `!commande reste` : renvoie le reste si `trigger` (éventuellement en
/// plusieurs mots) correspond au début du message, sans tenir compte de la casse.
pub fn match_trigger<'a>(body: &'a str, trigger: &str) -> Option<&'a str> {
    let mut rest = body;
    for word in trigger.split(' ') {
        let candidate = rest.trim_start();
        let end = candidate.find(char::is_whitespace).unwrap_or(candidate.len());
        if !candidate[..end].eq_ignore_ascii_case(word) {
            return None;
        }
        rest = &candidate[end..];
    }
    Some(rest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_signatures() {
        let params = [Param::Required("userId", Kind::UserId), Param::Optional("blockTime", Kind::Delay)];
        let args = parse(" 106391128718245888 7d", &params, &[]).unwrap();
        assert_eq!(args.str("userId"), Some("106391128718245888"));
        assert_eq!(args.delay("blockTime"), Some(604_800_000));
        assert!(parse(" 7d", &params, &[]).is_err());
        let args = parse(" 7d", &[Param::Optional("blockTime", Kind::Delay)], &[]).unwrap();
        assert_eq!(args.delay("blockTime"), Some(604_800_000));
    }

    #[test]
    fn keeps_rest_raw_and_reads_switches() {
        let args =
            parse(" hi  Hello \"you\"", &[Param::Required("trigger", Kind::Str), Param::Rest("text", false)], &[])
                .unwrap();
        assert_eq!(args.str("trigger"), Some("hi"));
        assert_eq!(args.str("text"), Some("Hello \"you\""));

        let args = parse(" -s 1h", &[Param::Variadic("opts")], &[("silent", "s"), ("cancel", "c")]).unwrap();
        assert!(args.switch("silent"));
        assert_eq!(args.list("opts"), vec!["1h".to_owned()]);
        assert!(parse(" extra", &[], &[]).is_err());
    }

    #[test]
    fn matches_multi_word_triggers() {
        assert_eq!(match_trigger("role reset", "role reset"), Some(""));
        assert_eq!(match_trigger("ROLE Staff", "role"), Some(" Staff"));
        assert_eq!(match_trigger("roles", "role"), None);
    }
}
