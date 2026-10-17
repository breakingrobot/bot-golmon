//! Rendu HTML du markdown Discord, sûr (tout le texte est échappé).
//!
//! Blocs : blocs de code (avec langage), titres `#`/`##`/`###`, sous-texte
//! `-#`, citations `>` et `>>>`, listes à puces et numérotées (imbriquées).
//! En ligne : gras, italique, souligné, barré, spoiler, code, liens masqués
//! `[texte](url)`, liens nus et `<url>`, mentions (résolues en noms quand c'est
//! possible), `@everyone`/`@here`, commandes `</nom:id>`, émojis personnalisés
//! et horodatages `<t:…:style>` (tous les styles, relatif compris).

use chrono::DateTime;
use std::fmt::Write as _;

/// Noms des mentions. Par défaut, rien n'est résolu (l'identifiant est affiché).
pub trait Resolver {
    fn user(&self, _id: u64) -> Option<String> {
        None
    }
    fn role(&self, _id: u64) -> Option<String> {
        None
    }
    fn channel(&self, _id: u64) -> Option<String> {
        None
    }
}

pub struct NoResolver;
impl Resolver for NoResolver {}

/// Message complet en HTML.
pub fn render(text: &str, resolver: &dyn Resolver) -> String {
    let mut code_blocks = Vec::new();
    let text = extract_code_blocks(text, &mut code_blocks);
    let lines: Vec<&str> = text.split('\n').collect();
    let mut html = String::new();
    render_blocks(&lines, resolver, &mut html);
    restore(&html, &code_blocks)
}

const OPEN: char = '\u{E000}';
const CLOSE: char = '\u{E001}';

fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

/// Les blocs de code sont remplacés par des repères avant tout le reste : rien
/// à l'intérieur n'est interprété.
fn extract_code_blocks(text: &str, blocks: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("```") {
        let after = &rest[start + 3..];
        let Some(end) = after.find("```") else {
            break;
        };
        let inner = &after[..end];
        // Langage : premier mot s'il est suivi d'un retour à la ligne.
        let (language, code) = match inner.split_once('\n') {
            Some((first, code))
                if !first.is_empty()
                    && first.len() <= 20
                    && first.chars().all(|c| c.is_ascii_alphanumeric() || "+-_#.".contains(c)) =>
            {
                (Some(first), code)
            }
            _ => (None, inner.strip_prefix('\n').unwrap_or(inner)),
        };
        let code = code.strip_suffix('\n').unwrap_or(code);
        let language_attr = language.map(|l| format!(r#" data-language="{}""#, escape(l))).unwrap_or_default();
        blocks.push(format!("<pre{language_attr}><code>{}</code></pre>", escape(code)));
        out.push_str(&rest[..start]);
        let _ = write!(out, "{OPEN}{}{CLOSE}", blocks.len() - 1);
        rest = &after[end + 3..];
    }
    out.push_str(rest);
    out
}

fn restore(html: &str, blocks: &[String]) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(start) = rest.find(OPEN) {
        out.push_str(&rest[..start]);
        let after = &rest[start + OPEN.len_utf8()..];
        match after.find(CLOSE) {
            Some(end) => {
                if let Some(block) = after[..end].parse::<usize>().ok().and_then(|i| blocks.get(i)) {
                    out.push_str(block);
                }
                rest = &after[end + CLOSE.len_utf8()..];
            }
            None => rest = after,
        }
    }
    out.push_str(rest);
    out
}

struct ListItem<'a> {
    depth: usize,
    ordered: Option<u32>,
    text: &'a str,
}

fn list_item(line: &str) -> Option<ListItem<'_>> {
    let trimmed = line.trim_start_matches(' ');
    let depth = (line.len() - trimmed.len()) / 2;
    if let Some(text) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
        return Some(ListItem { depth, ordered: None, text });
    }
    let digits = trimmed.bytes().take_while(u8::is_ascii_digit).count();
    if (1..=9).contains(&digits)
        && let Some(text) = trimmed[digits..].strip_prefix(". ")
    {
        return Some(ListItem { depth, ordered: trimmed[..digits].parse().ok(), text });
    }
    None
}

fn heading(line: &str) -> Option<(u8, &str)> {
    for (level, prefix) in [(3, "### "), (2, "## "), (1, "# ")] {
        if let Some(text) = line.strip_prefix(prefix).filter(|t| !t.trim().is_empty()) {
            return Some((level, text));
        }
    }
    None
}

fn render_blocks(lines: &[&str], resolver: &dyn Resolver, out: &mut String) {
    let mut paragraph: Vec<&str> = Vec::new();
    let flush = |paragraph: &mut Vec<&str>, out: &mut String| {
        if !paragraph.is_empty() {
            let rendered: Vec<String> = paragraph.iter().map(|l| inline(l, resolver)).collect();
            let _ = write!(out, "<p>{}</p>", rendered.join("<br>"));
            paragraph.clear();
        }
    };

    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if let Some(rest) = line.strip_prefix(">>> ") {
            flush(&mut paragraph, out);
            let mut quoted = vec![rest];
            quoted.extend_from_slice(&lines[i + 1..]);
            out.push_str("<blockquote>");
            render_blocks(&quoted, resolver, out);
            out.push_str("</blockquote>");
            return;
        }
        if line.starts_with("> ") || line == ">" {
            flush(&mut paragraph, out);
            let mut quoted = Vec::new();
            while i < lines.len() && (lines[i].starts_with("> ") || lines[i] == ">") {
                quoted.push(lines[i].strip_prefix("> ").unwrap_or(""));
                i += 1;
            }
            out.push_str("<blockquote>");
            render_blocks(&quoted, resolver, out);
            out.push_str("</blockquote>");
            continue;
        }
        if let Some((level, text)) = heading(line) {
            flush(&mut paragraph, out);
            let _ = write!(out, "<h{level}>{}</h{level}>", inline(text, resolver));
            i += 1;
            continue;
        }
        if let Some(text) = line.strip_prefix("-# ").filter(|t| !t.trim().is_empty()) {
            flush(&mut paragraph, out);
            let _ = write!(out, r#"<p class="subtext">{}</p>"#, inline(text, resolver));
            i += 1;
            continue;
        }
        if list_item(line).is_some() {
            flush(&mut paragraph, out);
            let mut items = Vec::new();
            while let Some(item) = lines.get(i).and_then(|l| list_item(l)) {
                items.push(item);
                i += 1;
            }
            render_list(&items, resolver, out);
            continue;
        }
        // Bloc de code seul sur sa ligne : hors paragraphe (un <pre> ne va pas dans un <p>).
        let trimmed = line.trim();
        if trimmed.starts_with(OPEN) && trimmed.ends_with(CLOSE) && trimmed.matches(OPEN).count() == 1 {
            flush(&mut paragraph, out);
            out.push_str(trimmed);
            i += 1;
            continue;
        }
        if trimmed.is_empty() {
            flush(&mut paragraph, out);
            i += 1;
            continue;
        }
        paragraph.push(line);
        i += 1;
    }
    flush(&mut paragraph, out);
}

/// Listes imbriquées : chaque élément plus indenté que le précédent ouvre une sous-liste.
fn render_list(items: &[ListItem<'_>], resolver: &dyn Resolver, out: &mut String) {
    let mut stack: Vec<(usize, &'static str)> = Vec::new();
    for item in items {
        let tag = if item.ordered.is_some() { "ol" } else { "ul" };
        while stack.last().is_some_and(|(depth, _)| *depth > item.depth) {
            let (_, closing) = stack.pop().expect("pile non vide");
            let _ = write!(out, "</li></{closing}>");
        }
        // Même niveau mais autre type de liste : la liste précédente se termine.
        if stack.last().is_some_and(|(depth, open)| *depth == item.depth && *open != tag) {
            let (_, closing) = stack.pop().expect("pile non vide");
            let _ = write!(out, "</li></{closing}>");
        }
        match stack.last() {
            Some((depth, _)) if *depth == item.depth => out.push_str("</li>"),
            _ => {
                match item.ordered.filter(|start| *start != 1) {
                    Some(start) => {
                        let _ = write!(out, r#"<{tag} start="{start}">"#);
                    }
                    None => {
                        let _ = write!(out, "<{tag}>");
                    }
                }
                stack.push((item.depth, tag));
            }
        }
        let _ = write!(out, "<li>{}", inline(item.text, resolver));
    }
    while let Some((_, closing)) = stack.pop() {
        let _ = write!(out, "</li></{closing}>");
    }
}

fn is_url_char(c: char) -> bool {
    !c.is_whitespace() && !matches!(c, '<' | '>' | '"' | '\'' | OPEN | CLOSE)
}

fn safe_url(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://")) && url.len() > 8 && url.chars().all(is_url_char)
}

fn link(url: &str, label_html: &str) -> String {
    format!(r#"<a href="{}" target="_blank" rel="noopener noreferrer nofollow">{label_html}</a>"#, escape(url))
}

/// Lien nu : la ponctuation finale ne fait pas partie de l'URL (sauf parenthèse équilibrée).
fn bare_url_len(text: &str) -> usize {
    let mut end = text.find(|c: char| !is_url_char(c)).unwrap_or(text.len());
    while end > 0 {
        let last = text[..end].chars().last().expect("non vide");
        let unbalanced = last == ')' && text[..end].matches('(').count() < text[..end].matches(')').count();
        if ".,:;!?".contains(last) || unbalanced {
            end -= last.len_utf8();
        } else {
            break;
        }
    }
    end
}

fn timestamp(seconds: i64, style: char) -> Option<String> {
    let date = DateTime::from_timestamp(seconds, 0)?;
    let fallback = match style {
        't' => date.format("%H:%M UTC").to_string(),
        'T' => date.format("%H:%M:%S UTC").to_string(),
        'd' => date.format("%d/%m/%Y").to_string(),
        'D' => date.format("%d/%m/%Y").to_string(),
        'R' => date.format("%d/%m/%Y %H:%M UTC").to_string(),
        _ => date.format("%d/%m/%Y %H:%M UTC").to_string(),
    };
    Some(format!(
        r#"<time class="timestamp" datetime="{}" data-local data-format="{style}">{fallback}</time>"#,
        date.to_rfc3339()
    ))
}

/// `<…>` : mentions, émojis, horodatages, commandes, liens sans aperçu.
fn angle(inner: &str, resolver: &dyn Resolver) -> Option<String> {
    let id = |s: &str| s.parse::<u64>().ok().filter(|_| s.len() >= 15);
    if let Some(rest) = inner.strip_prefix("t:") {
        let (seconds, style) = rest.split_once(':').map_or((rest, 'f'), |(s, f)| (s, f.chars().next().unwrap_or('f')));
        if !"tTdDfFR".contains(style) {
            return None;
        }
        return timestamp(seconds.parse().ok()?, style);
    }
    if let Some(rest) = inner.strip_prefix("@&") {
        let role = id(rest)?;
        let name = resolver.role(role).unwrap_or_else(|| rest.to_owned());
        return Some(format!(r#"<span class="mention">@{}</span>"#, escape(&name)));
    }
    if let Some(rest) = inner.strip_prefix('@') {
        let user = id(rest.strip_prefix('!').unwrap_or(rest))?;
        let name = resolver.user(user).unwrap_or_else(|| user.to_string());
        return Some(format!(r#"<span class="mention" title="{user}">@{}</span>"#, escape(&name)));
    }
    if let Some(rest) = inner.strip_prefix('#') {
        let channel = id(rest)?;
        let name = resolver.channel(channel).unwrap_or_else(|| channel.to_string());
        return Some(format!(r#"<span class="mention">#{}</span>"#, escape(&name)));
    }
    if let Some(rest) = inner.strip_prefix('/') {
        let (name, command_id) = rest.rsplit_once(':')?;
        id(command_id)?;
        return Some(format!(r#"<span class="mention">/{}</span>"#, escape(name)));
    }
    let (animated, rest) = match inner.strip_prefix("a:") {
        Some(rest) => (true, rest),
        None => (false, inner.strip_prefix(':')?),
    };
    let (name, emoji_id) = rest.split_once(':')?;
    let emoji_id = id(emoji_id)?;
    if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    let ext = if animated { "gif" } else { "webp" };
    Some(format!(
        r#"<img class="emoji" src="https://cdn.discordapp.com/emojis/{emoji_id}.{ext}?size=48" alt=":{name}:" title=":{name}:">"#
    ))
}

/// Délimiteurs symétriques : (marqueur, balise ouvrante, balise fermante).
const DELIMITERS: [(&str, &str, &str); 6] = [
    ("***", "<strong><em>", "</em></strong>"),
    ("**", "<strong>", "</strong>"),
    ("__", "<u>", "</u>"),
    ("~~", "<s>", "</s>"),
    ("||", r#"<span class="spoiler" tabindex="0">"#, "</span>"),
    ("*", "<em>", "</em>"),
];

/// Mise en forme en ligne.
fn inline(text: &str, resolver: &dyn Resolver) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut i = 0;
    'scan: while i < text.len() {
        let rest = &text[i..];
        let ch = rest.chars().next().expect("non vide");

        // Échappement : `\*` affiche une étoile.
        if ch == '\\'
            && let Some(next) = rest[1..].chars().next()
            && next.is_ascii_punctuation()
        {
            out.push_str(&escape(&next.to_string()));
            i += 1 + next.len_utf8();
            continue;
        }

        // Repère de bloc de code : recopié tel quel.
        if ch == OPEN
            && let Some(end) = rest.find(CLOSE)
        {
            out.push_str(&rest[..end + CLOSE.len_utf8()]);
            i += end + CLOSE.len_utf8();
            continue;
        }

        // Code en ligne : même nombre de backticks à l'ouverture et à la fermeture.
        if ch == '`' {
            let ticks = rest.bytes().take_while(|b| *b == b'`').count();
            let fence = &rest[..ticks];
            if let Some(end) = rest[ticks..].find(fence).filter(|end| *end > 0) {
                let code = &rest[ticks..ticks + end];
                let code = if ticks > 1 { code.strip_prefix(' ').unwrap_or(code) } else { code };
                let code = if ticks > 1 { code.strip_suffix(' ').unwrap_or(code) } else { code };
                let _ = write!(out, "<code>{}</code>", escape(code));
                i += ticks * 2 + end;
                continue;
            }
            out.push_str(fence);
            i += ticks;
            continue;
        }

        if ch == '<'
            && let Some(end) = rest.find('>')
        {
            let inner = &rest[1..end];
            if safe_url(inner) {
                out.push_str(&link(inner, &escape(inner)));
                i += end + 1;
                continue;
            }
            if let Some(html) = angle(inner, resolver) {
                out.push_str(&html);
                i += end + 1;
                continue;
            }
        }

        // Lien masqué : [texte](https://…)
        if ch == '['
            && let Some(close) = rest.find("](")
            && let Some(paren) = rest[close + 2..].find(')')
        {
            let label = &rest[1..close];
            let url = rest[close + 2..close + 2 + paren].trim_matches(|c| c == '<' || c == '>');
            if !label.trim().is_empty() && !label.contains('\n') && safe_url(url) {
                out.push_str(&link(url, &inline(label, resolver)));
                i += close + 3 + paren;
                continue;
            }
        }

        if rest.starts_with("https://") || rest.starts_with("http://") {
            let len = bare_url_len(rest);
            if safe_url(&rest[..len]) {
                out.push_str(&link(&rest[..len], &escape(&rest[..len])));
                i += len;
                continue;
            }
        }

        for mention in ["@everyone", "@here"] {
            if rest.starts_with(mention) {
                let _ = write!(out, r#"<span class="mention">{mention}</span>"#);
                i += mention.len();
                continue 'scan;
            }
        }

        for (marker, open, close) in DELIMITERS {
            if let Some(inner) = rest.strip_prefix(marker)
                && let Some(end) = find_closing(inner, marker)
            {
                let _ = write!(out, "{open}{}{close}", inline(&inner[..end], resolver));
                i += marker.len() * 2 + end;
                continue 'scan;
            }
        }

        // `_italique_` : seulement en début de mot, fermé en fin de mot.
        if ch == '_' && !text[..i].chars().last().is_some_and(char::is_alphanumeric) {
            let inner = &rest[1..];
            if let Some(end) = find_closing(inner, "_")
                && !inner[end + 1..].chars().next().is_some_and(char::is_alphanumeric)
            {
                let _ = write!(out, "<em>{}</em>", inline(&inner[..end], resolver));
                i += end + 2;
                continue;
            }
        }

        out.push_str(&escape(&ch.to_string()));
        i += ch.len_utf8();
    }
    out
}

/// Position du marqueur fermant (règles de Discord) : contenu non vide, marqueur
/// non suivi du même caractère (`**` se ferme sur la fin d'un `***`), hors code
/// en ligne et caractères échappés ; pour l'italique, pas d'espace en bordure.
fn find_closing(inner: &str, marker: &str) -> Option<usize> {
    let single = marker == "*" || marker == "_";
    if single && inner.starts_with(char::is_whitespace) {
        return None;
    }
    let first = marker.chars().next()?;
    let mut index = 0;
    let mut in_code = false;
    while index < inner.len() {
        let rest = &inner[index..];
        let c = rest.chars().next()?;
        if c == '`' {
            in_code = !in_code;
        } else if !in_code {
            if c == '\\' {
                index += 1 + rest[1..].chars().next().map_or(0, char::len_utf8);
                continue;
            }
            // Un gras à l'intérieur d'un italique se saute d'un bloc.
            if marker == "*" && rest.starts_with("**") {
                index += 2;
                continue;
            }
            if index > 0 && rest.starts_with(marker) {
                let followed = marker != "||" && rest[marker.len()..].starts_with(first);
                let space_before = single && inner[..index].ends_with(char::is_whitespace);
                if !followed && !space_before {
                    return Some(index);
                }
            }
        }
        index += c.len_utf8();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(text: &str) -> String {
        render(text, &NoResolver)
    }

    struct Names;
    impl Resolver for Names {
        fn user(&self, _: u64) -> Option<String> {
            Some("Élodie".into())
        }
        fn role(&self, _: u64) -> Option<String> {
            Some("Modération".into())
        }
        fn channel(&self, _: u64) -> Option<String> {
            Some("général".into())
        }
    }

    #[test]
    fn escapes_html() {
        let html = md("<script>alert(1)</script> & \"x\"");
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;") && html.contains("&amp;") && html.contains("&quot;"));
    }

    #[test]
    fn inline_styles() {
        assert_eq!(md("**gras**"), "<p><strong>gras</strong></p>");
        assert_eq!(md("*it* _it_"), "<p><em>it</em> <em>it</em></p>");
        assert_eq!(md("***les deux***"), "<p><strong><em>les deux</em></strong></p>");
        assert_eq!(md("__sous__ ~~barré~~"), "<p><u>sous</u> <s>barré</s></p>");
        assert_eq!(md("**gras *et italique***"), "<p><strong>gras <em>et italique</em></strong></p>");
        assert_eq!(md("__**souligné gras**__"), "<p><u><strong>souligné gras</strong></u></p>");
        assert!(md("||secret||").contains(r#"<span class="spoiler" tabindex="0">secret</span>"#));
        assert_eq!(md("snake_case_name et 2*3*4"), "<p>snake_case_name et 2<em>3</em>4</p>");
        assert_eq!(md(r"\*pas italique\*"), "<p>*pas italique*</p>");
        assert_eq!(md("a * b * c"), "<p>a * b * c</p>");
    }

    #[test]
    fn code() {
        assert_eq!(md("`**x**`"), "<p><code>**x**</code></p>");
        assert_eq!(md("``a ` b``"), "<p><code>a ` b</code></p>");
        assert_eq!(md("```rust\nlet a = 1;\n```"), r#"<pre data-language="rust"><code>let a = 1;</code></pre>"#);
        assert!(md("```\n# pas un titre\n```").contains("<code># pas un titre</code>"));
    }

    #[test]
    fn blocks() {
        assert_eq!(md("# Titre\n## Sous\n### Petit"), "<h1>Titre</h1><h2>Sous</h2><h3>Petit</h3>");
        assert_eq!(md("#pas un titre"), "<p>#pas un titre</p>");
        assert_eq!(md("-# note"), r#"<p class="subtext">note</p>"#);
        assert_eq!(md("> a\n> b\nc"), "<blockquote><p>a<br>b</p></blockquote><p>c</p>");
        assert_eq!(md("avant\n>>> a\nb"), "<p>avant</p><blockquote><p>a<br>b</p></blockquote>");
        assert_eq!(md("> # titre cité"), "<blockquote><h1>titre cité</h1></blockquote>");
    }

    #[test]
    fn lists() {
        assert_eq!(md("- a\n- b"), "<ul><li>a</li><li>b</li></ul>");
        assert_eq!(md("1. a\n2. b"), "<ol><li>a</li><li>b</li></ol>");
        assert_eq!(md("3. a"), r#"<ol start="3"><li>a</li></ol>"#);
        assert_eq!(md("- a\n  - b\n- c"), "<ul><li>a<ul><li>b</li></ul></li><li>c</li></ul>");
        assert_eq!(md("- a\n1. b"), "<ul><li>a</li></ul><ol><li>b</li></ol>");
    }

    #[test]
    fn links() {
        assert_eq!(
            md("[le **site**](https://example.com/a)"),
            r#"<p><a href="https://example.com/a" target="_blank" rel="noopener noreferrer nofollow">le <strong>site</strong></a></p>"#
        );
        assert!(md("[x](javascript:alert(1))").contains("[x](javascript:alert(1))"));
        let html = md("voir https://example.com/a_b?c=1. ok");
        assert!(html.contains(r#"href="https://example.com/a_b?c=1""#) && html.contains("</a>. ok"));
        assert!(md("<https://example.com>").contains(r#"href="https://example.com""#));
        assert!(md("(https://fr.wikipedia.org/wiki/A_(b))").contains(r#"href="https://fr.wikipedia.org/wiki/A_(b)""#));
    }

    #[test]
    fn mentions_emojis_timestamps() {
        let html = render("<@106391128718245888> <@&123456789012345678> <#123456789012345678> @everyone", &Names);
        assert!(html.contains("@Élodie") && html.contains("@Modération") && html.contains("#général"));
        assert!(html.contains(r#"<span class="mention">@everyone</span>"#));
        assert!(md("<@106391128718245888>").contains("@106391128718245888"));
        assert!(md("</ping:123456789012345678>").contains("/ping"));
        assert!(md("<:salut:123456789012345678>").contains("emojis/123456789012345678.webp"));
        assert!(md("<a:dance:123456789012345678>").contains("emojis/123456789012345678.gif"));
        let html = md("<t:1700000000:R> <t:1700000000>");
        assert!(html.contains(r#"data-format="R""#) && html.contains(r#"data-format="f""#));
        assert!(html.contains("2023-11-14T22:13:20+00:00"));
    }
}
