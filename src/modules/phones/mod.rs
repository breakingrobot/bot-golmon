pub mod commands;
pub mod interactions;
pub mod service;

pub const NAMESPACE: &str = "numero";

pub const INVALID_FORMAT: &str =
    "Format invalide. Un numéro commence par 555 suivi de 4 à 6 chiffres, par exemple `5551234`.";

/// Ne garde que les chiffres : `555-1234` et `5551234` sont équivalents.
pub fn normalize(input: &str) -> String {
    input.chars().filter(char::is_ascii_digit).collect()
}

/// Format RP attendu : 555 suivi de 4 à 6 chiffres (555XXXX à 555XXXXXX).
pub fn is_valid(numero: &str) -> bool {
    numero.starts_with("555") && (7..=9).contains(&numero.len()) && numero.bytes().all(|b| b.is_ascii_digit())
}

pub fn format(numero: &str) -> String {
    match numero.strip_prefix("555") {
        Some(rest) => format!("555-{rest}"),
        None => numero.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_phone_numbers() {
        assert!(is_valid(&normalize("555-1234")));
        assert!(is_valid("555123456"));
        assert!(!is_valid("5551234567"));
        assert!(!is_valid("556123"));
        assert_eq!(format("5551234"), "555-1234");
    }
}
