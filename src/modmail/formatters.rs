//! Mise en forme des messages, identique à `src/formatters.js` de modmailbot.

use super::{
    config::MmConfig,
    models::{Thread, ThreadMessage, message_type},
    util::{disable_code_blocks, disable_inline_code, parse_db_time},
};

fn role_name<'a>(config: &'a MmConfig, message: &'a ThreadMessage) -> Option<&'a str> {
    config
        .override_role_name_display
        .as_deref()
        .or(message.role_name.as_deref())
        .or(config.fallback_role_name.as_deref())
}

fn timestamp(created_at: &str) -> String {
    parse_db_time(created_at).map(|d| d.format("%H:%M").to_string()).unwrap_or_default()
}

/// Réponse du staff telle que reçue en message privé par l'utilisateur.
pub fn staff_reply_dm(config: &MmConfig, message: &ThreadMessage) -> String {
    let role = role_name(config, message);
    let mod_info = if message.is_anonymous != 0 {
        role.map(str::to_owned)
    } else {
        Some(match role {
            Some(role) => format!("({role}) {}", message.user_name),
            None => message.user_name.clone(),
        })
    };
    match mod_info.filter(|info| !info.is_empty()) {
        Some(info) => format!("**{info}:** {}", message.body()),
        None => message.body().to_owned(),
    }
}

/// Réponse du staff telle qu'affichée dans le salon du ticket.
pub fn staff_reply_thread_message(config: &MmConfig, message: &ThreadMessage) -> String {
    let role = role_name(config, message);
    let mod_info = if message.is_anonymous != 0 {
        match role {
            Some(role) => format!("(Anonymous) ({}) {role}", message.user_name),
            None => format!("(Anonymous) ({})", message.user_name),
        }
    } else {
        match role {
            Some(role) => format!("({role}) {}", message.user_name),
            None => message.user_name.clone(),
        }
    };
    let mut result =
        if mod_info.is_empty() { message.body().to_owned() } else { format!("**{mod_info}:** {}", message.body()) };
    if config.thread_timestamps {
        result = format!("[{}] {result}", timestamp(&message.created_at));
    }
    format!("`{}`  {result}", message.message_number.unwrap_or_default())
}

pub fn user_reply_thread_message(config: &MmConfig, message: &ThreadMessage) -> String {
    let mut result = format!("**{}:** {}", message.user_name, message.body());
    for link in message.attachment_links() {
        result.push_str(&format!("\n\n{link}"));
    }
    if config.thread_timestamps {
        result = format!("[{}] {result}", timestamp(&message.created_at));
    }
    result
}

pub fn staff_reply_edit_notification(original: &ThreadMessage, new_body: &str) -> String {
    let mut content = format!(
        "**{}** (`{}`) edited reply `{}`",
        original.user_name,
        original.user_id.as_deref().unwrap_or_default(),
        original.message_number.unwrap_or_default()
    );
    let old_body = original.body();
    if old_body.encode_utf16().count() < 200 && new_body.encode_utf16().count() < 200 {
        // Petites modifications affichées en ligne.
        content.push_str(&format!(" from `{}` to `{new_body}`", disable_inline_code(old_body)));
    } else {
        content.push(':');
        content.push_str(&format!("\n\nBefore:\n```{}```", disable_code_blocks(old_body)));
        content.push_str(&format!("\nAfter:\n```{}```", disable_code_blocks(new_body)));
    }
    content
}

pub fn staff_reply_deletion_notification(original: &ThreadMessage) -> String {
    let mut content = format!(
        "**{}** (`{}`) deleted reply `{}`",
        original.user_name,
        original.user_id.as_deref().unwrap_or_default(),
        original.message_number.unwrap_or_default()
    );
    let body = original.body();
    if body.encode_utf16().count() < 200 {
        content.push_str(&format!(" (message content: `{}`)", disable_inline_code(body)));
    } else {
        content.push_str(&format!(":\n```{}```", disable_code_blocks(body)));
    }
    content
}

pub fn system_thread_message(message: &ThreadMessage) -> String {
    let mut result = message.body().to_owned();
    for link in message.attachment_links() {
        result.push_str(&format!("\n\n{link}"));
    }
    result
}

pub fn system_to_user_thread_message(bot_name: &str, body: &str) -> String {
    format!("**⚙️ {bot_name}:** {body}")
}

#[derive(Clone, Copy, Default)]
pub struct LogOptions {
    pub simple: bool,
    pub verbose: bool,
}

/// Le message est-il visible dans la vue « simple » (échanges avec l'utilisateur uniquement) ?
pub fn shown_in_simple_view(message: &ThreadMessage) -> bool {
    !matches!(
        message.message_type,
        message_type::SYSTEM | message_type::SYSTEM_TO_USER | message_type::CHAT | message_type::COMMAND
    )
}

/// Log texte d'un ticket, au format exact de modmailbot.
pub fn format_log(thread: &Thread, messages: &[ThreadMessage], options: LogOptions) -> String {
    let lines: Vec<String> = messages
        .iter()
        .filter(|m| !options.simple || shown_in_simple_view(m))
        .map(|message| {
            // Messages hérités (2018) : le log entier tient dans un message.
            if message.message_type == message_type::LEGACY {
                return message.body().to_owned();
            }
            let time = parse_db_time(&message.created_at)
                .map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_default();
            let mut line = format!("[{time}]");
            if options.verbose {
                if let Some(id) = &message.dm_channel_id {
                    line.push_str(&format!(" [DM CHA {id}]"));
                }
                if let Some(id) = &message.dm_message_id {
                    line.push_str(&format!(" [DM MSG {id}]"));
                }
            }

            let body = message.body();
            match message.message_type {
                message_type::FROM_USER => line.push_str(&format!(" [FROM USER] [{}] {body}", message.user_name)),
                message_type::TO_USER => {
                    if options.verbose {
                        line.push_str(&format!(
                            " [TO USER] [{}] [{}]",
                            message.message_number.unwrap_or(0),
                            message.user_name
                        ));
                    } else {
                        line.push_str(&format!(" [TO USER] [{}]", message.user_name));
                    }
                    if message.use_legacy_format.unwrap_or(0) != 0 {
                        line.push_str(&format!(" {body}"));
                    } else if message.is_anonymous != 0 {
                        match &message.role_name {
                            Some(role) => line.push_str(&format!(" (Anonymous) {role}: {body}")),
                            None => line.push_str(&format!(" (Anonymous) Moderator: {body}")),
                        }
                    } else {
                        match &message.role_name {
                            Some(role) => line.push_str(&format!(" ({role}) {}: {body}", message.user_name)),
                            None => line.push_str(&format!(" {}: {body}", message.user_name)),
                        }
                    }
                }
                message_type::SYSTEM => line.push_str(&format!(" [BOT] {body}")),
                message_type::SYSTEM_TO_USER => line.push_str(&format!(" [BOT TO USER] {body}")),
                message_type::CHAT => line.push_str(&format!(" [CHAT] [{}] {body}", message.user_name)),
                message_type::COMMAND => line.push_str(&format!(" [COMMAND] [{}] {body}", message.user_name)),
                message_type::REPLY_EDITED => {
                    let original = message.original().unwrap_or_default();
                    let new_body = message
                        .metadata_value("newBody")
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default();
                    line.push_str(&format!(
                        " [REPLY EDITED] {} edited reply {}:\n\nBefore:\n{}\n\nAfter:\n{new_body}",
                        original.user_name,
                        original.message_number.unwrap_or_default(),
                        original.body()
                    ));
                }
                message_type::REPLY_DELETED => {
                    let original = message.original().unwrap_or_default();
                    line.push_str(&format!(
                        " [REPLY DELETED] {} deleted reply {}:\n\n{}",
                        original.user_name,
                        original.message_number.unwrap_or_default(),
                        original.body()
                    ));
                }
                _ => line.push_str(&format!(" [{}] {body}", message.user_name)),
            }

            let links = message.attachment_links();
            if !links.is_empty() {
                line.push_str("\n\n");
                line.push_str(&links.join("\n"));
            }
            line
        })
        .collect();

    let opened =
        parse_db_time(&thread.created_at).map(|d| d.format("%Y-%m-%d %H:%M:%S").to_string()).unwrap_or_default();
    format!(
        "# Modmail thread #{} with {} ({}) started at {opened}. All times are in UTC+0.\n\n{}",
        thread.number(),
        thread.user_name,
        thread.user_id,
        lines.join("\n")
    )
}

/// Résumé texte des embeds (pages de logs), comme `embedLogging.js`.
pub fn summarise_embeds(embeds: Option<&serde_json::Value>, label: &str) -> String {
    let Some(list) = embeds.and_then(|v| v.as_array()).filter(|l| !l.is_empty()) else {
        return String::new();
    };
    let list: Vec<&serde_json::Value> = list.iter().filter(|e| !e.is_null()).take(10).collect();
    let mut lines = vec![format!("*{label}:*")];
    let text = |value: Option<&serde_json::Value>| {
        value.and_then(|v| v.as_str()).map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
    };
    for (index, embed) in list.iter().enumerate() {
        if list.len() > 1 {
            lines.push(format!("Embed {}:", index + 1));
        }
        if let Some(title) = text(embed.get("title")) {
            lines.push(format!("Title: {title}"));
        }
        if let Some(url) = text(embed.get("url")) {
            lines.push(format!("URL: {url}"));
        }
        if let Some(description) = text(embed.get("description")) {
            lines.push(format!("Description: {description}"));
        }
        for (key, label) in [("image", "Image"), ("thumbnail", "Thumbnail"), ("video", "Video")] {
            if let Some(url) = text(embed.get(key).and_then(|m| m.get("url"))) {
                lines.push(format!("{label}: {url}"));
            }
        }
        if let Some(fields) = embed.get("fields").and_then(|f| f.as_array()) {
            for field in fields.iter().take(25) {
                if let Some(name) = text(field.get("name")) {
                    lines.push(format!("Field: {name}"));
                }
                if let Some(value) = text(field.get("value")) {
                    lines.push(value);
                }
            }
        }
        if list.len() > 1 && index != list.len() - 1 {
            lines.push(String::new());
        }
    }
    lines.join("\n").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn message(kind: i64, body: &str) -> ThreadMessage {
        ThreadMessage {
            message_type: kind,
            user_name: "Mod".into(),
            user_id: Some("1".into()),
            body: Some(body.into()),
            created_at: "2026-01-02 03:04:05".into(),
            message_number: Some(3),
            role_name: Some("Staff".into()),
            ..Default::default()
        }
    }

    #[test]
    fn formats_log_lines_like_modmailbot() {
        let thread = Thread {
            id: "t".into(),
            status: 2,
            user_id: "42".into(),
            user_name: "user".into(),
            channel_id: None,
            created_at: "2026-01-02 03:00:00".into(),
            scheduled_close_at: None,
            scheduled_close_id: None,
            scheduled_close_name: None,
            scheduled_close_silent: None,
            scheduled_suspend_at: None,
            scheduled_suspend_name: None,
            alert_ids: None,
            log_storage_type: None,
            log_storage_data: None,
            thread_number: Some(7),
        };
        let messages = vec![message(message_type::TO_USER, "hello"), message(message_type::CHAT, "internal")];
        let log = format_log(&thread, &messages, LogOptions::default());
        assert!(
            log.starts_with(
                "# Modmail thread #7 with user (42) started at 2026-01-02 03:00:00. All times are in UTC+0."
            )
        );
        assert!(log.contains("[2026-01-02 03:04:05] [TO USER] [Mod] (Staff) Mod: hello"));
        assert!(log.contains("[CHAT] [Mod] internal"));
        let simple = format_log(&thread, &messages, LogOptions { simple: true, verbose: false });
        assert!(!simple.contains("internal"));
    }
}
