//! Lecture des fichiers `config.ini` au comportement du paquet npm `ini`
//! (celui qu'utilise modmailbot) : sections imbriquées, `cle[] = valeur` pour
//! les tableaux, guillemets optionnels, commentaires `;` et `#`.

use serde_json::{Map, Value};

fn strip_comment(value: &str) -> String {
    let mut out = String::new();
    let mut escaped = false;
    for ch in value.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
        } else if ch == '\\' {
            escaped = true;
        } else if ch == ';' || ch == '#' {
            break;
        } else {
            out.push(ch);
        }
    }
    out.trim().to_owned()
}

/// Valeur brute → valeur typée, comme `ini.unsafe` + la coercition de `ini.decode`.
fn parse_value(raw: &str) -> Value {
    let raw = raw.trim();
    let is_quoted = raw.len() >= 2
        && ((raw.starts_with('"') && raw.ends_with('"')) || (raw.starts_with('\'') && raw.ends_with('\'')));
    if is_quoted {
        let inner = &raw[1..raw.len() - 1];
        // JSON pour gérer les échappements des chaînes entre guillemets doubles.
        if raw.starts_with('"')
            && let Ok(Value::String(parsed)) = serde_json::from_str::<Value>(raw)
        {
            return Value::String(parsed);
        }
        return Value::String(inner.to_owned());
    }
    let value = strip_comment(raw);
    match value.as_str() {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        "null" => Value::Null,
        _ => Value::String(value),
    }
}

fn section_mut<'a>(root: &'a mut Map<String, Value>, path: &[String]) -> &'a mut Map<String, Value> {
    let mut cursor = root;
    for part in path {
        let entry = cursor.entry(part.clone()).or_insert_with(|| Value::Object(Map::new()));
        if !entry.is_object() {
            *entry = Value::Object(Map::new());
        }
        cursor = entry.as_object_mut().expect("objet");
    }
    cursor
}

pub fn decode(source: &str) -> Map<String, Value> {
    let mut root = Map::new();
    let mut section: Vec<String> = Vec::new();

    for line in source.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            let name = line[1..line.len() - 1].trim();
            section = name.split('.').map(str::to_owned).collect();
            continue;
        }

        let (key, raw_value) = match line.split_once('=') {
            Some((key, value)) => (key.trim(), Some(value)),
            None => (line, None),
        };
        let value = raw_value.map_or(Value::Bool(true), parse_value);
        let target = section_mut(&mut root, &section);

        if let Some(array_key) = key.strip_suffix("[]") {
            let entry = target.entry(array_key.to_owned()).or_insert_with(|| Value::Array(Vec::new()));
            match entry {
                Value::Array(items) => items.push(value),
                other => *other = Value::Array(vec![other.clone(), value]),
            }
        } else {
            target.insert(key.to_owned(), value);
        }
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_modmail_style_ini() {
        let source = r#"
# Commentaire
token = abc
mainServerId[] = 111
mainServerId[] = 222
status = Message me for help! ; commentaire
allowMove = true
responseMessage[] = Ligne 1
responseMessage[] = Ligne 2
quoted = "a ; b"
categoryAutomation.newThreadFromServer.111 = 999

[logOptions]
attachmentDirectory = logs
"#;
        let map = decode(source);
        assert_eq!(map["token"], "abc");
        assert_eq!(map["mainServerId"], serde_json::json!(["111", "222"]));
        assert_eq!(map["status"], "Message me for help!");
        assert_eq!(map["allowMove"], true);
        assert_eq!(map["quoted"], "a ; b");
        assert_eq!(map["categoryAutomation.newThreadFromServer.111"], "999");
        assert_eq!(map["logOptions"]["attachmentDirectory"], "logs");
    }
}
