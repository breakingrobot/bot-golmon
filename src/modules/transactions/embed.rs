use super::model::{Status, Transaction, type_label};
use crate::{
    discord::{
        components::{ButtonExt as _, button, container, row, separator, text_display, text_displays},
        embed::{EmbedBuilder, footer, timestamp},
    },
    util::{
        format::{ListBlock, V2_TEXT_LIMIT, field, format_price, pack_blocks, plural, truncate_field},
        ids::encode_custom_id,
        time::DbTime,
    },
};
use twilight_model::channel::message::{Component, Embed, component::ButtonStyle};

pub fn build_embed(transaction: &Transaction) -> Embed {
    let status = transaction.status();
    let items = if transaction.items.is_empty() {
        "Aucun objet".to_owned()
    } else {
        transaction
            .items
            .iter()
            .map(|item| {
                let unit = item
                    .prix_unitaire
                    .filter(|p| *p != 0.0)
                    .map(|p| format!(" — {}", format_price(p)))
                    .unwrap_or_default();
                format!("• {} ×{}{unit}", item.nom, item.quantite)
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    EmbedBuilder::new()
        .title(crate::util::format::truncate(&format!("Transaction — {}", transaction.groupe), 256))
        .color(status.color())
        .field(field("Type", type_label(&transaction.kind), true))
        .field(field("Prix", format_price(transaction.prix), true))
        .field(field("Statut", format!("{} {}", status.emoji(), status.label()), true))
        .field(field(format!("Objets ({})", transaction.items.len()), truncate_field(&items), false))
        .field(field("Créée par", format!("<@{}>", transaction.created_by_id), true))
        .footer(footer(&transaction.id))
        .timestamp(timestamp(transaction.updated_at))
        .build()
}

pub fn build_buttons(transaction_id: &str, current: Status) -> Vec<Component> {
    if current == Status::Cloturee {
        return Vec::new();
    }
    let status_button = |status: Status, label: &str, style: ButtonStyle| {
        // Le statut courant est désactivé : le clic serait sans effet.
        Component::Button(
            button(encode_custom_id("transaction", "status", transaction_id, Some(status.as_str())), label, style)
                .disabled(status == current),
        )
    };

    let mut status_row = vec![
        status_button(Status::EnAttenteRdv, "En attente RDV", ButtonStyle::Primary),
        status_button(Status::AvanceAttentePaiement, "Avance", ButtonStyle::Primary),
        status_button(Status::LivrePaye, "Livré et payé", ButtonStyle::Success),
        status_button(Status::Annulee, "Annuler", ButtonStyle::Danger),
    ];
    if current == Status::LivrePaye {
        status_row.push(Component::Button(
            button(
                encode_custom_id("transaction", "status", transaction_id, Some(Status::Cloturee.as_str())),
                "Clôturer",
                ButtonStyle::Secondary,
            )
            .with_emoji("🔒"),
        ));
    }

    let actions = vec![
        Component::Button(
            button(
                encode_custom_id("transaction", "additem", transaction_id, None),
                "Ajouter un objet",
                ButtonStyle::Secondary,
            )
            .with_emoji("➕"),
        ),
        Component::Button(
            button(encode_custom_id("transaction", "delete", transaction_id, None), "Supprimer", ButtonStyle::Danger)
                .with_emoji("🗑️"),
        ),
    ];

    vec![row(status_row), row(actions)]
}

/// Titre, résumé et pied de page sont décomptés à part ; on garde une marge.
const TEXT_BUDGET: usize = V2_TEXT_LIMIT - 100;
const ACCENT_COLOR: u32 = 0x5865f2;

fn format_line(transaction: &Transaction) -> String {
    let count = transaction.items.len();
    let items = if count > 0 { format!(" · {count} objet{}", plural(count)) } else { String::new() };
    format!(
        "- **{}** · {} · {}{items}",
        transaction.groupe,
        type_label(&transaction.kind),
        format_price(transaction.prix)
    )
}

fn status_block(transactions: &[Transaction], status: Status) -> ListBlock {
    let lines: Vec<String> = transactions.iter().filter(|t| t.status() == status).map(format_line).collect();
    ListBlock {
        header: format!("### {} {} ({})", status.emoji(), status.label(), lines.len()),
        lines,
        empty: "-# Aucune".to_owned(),
    }
}

/// Tableau de bord en Components V2 : les transactions en cours d'abord, puis
/// les dernières terminées, dans la limite de texte d'un message. `total`
/// compte toutes les transactions de la guilde, y compris celles écartées.
pub fn build_dashboard(transactions: &[Transaction], total: i64) -> Vec<Component> {
    let active: Vec<&Transaction> = transactions.iter().filter(|t| Status::ACTIVE.contains(&t.status())).collect();
    let engaged: f64 = active.iter().map(|t| t.prix).sum();
    let header = format!(
        "## 📊 Transactions\n**{}** transaction(s) en cours · **{}** engagé(s) · **{total}** au total",
        active.len(),
        format_price(engaged)
    );
    let footer =
        format!("-# Mis à jour <t:{}:R> · détail d'une transaction : `/transaction view`", DbTime::now().unix());

    let blocks: Vec<ListBlock> =
        Status::ACTIVE.iter().chain(&Status::TERMINAL).map(|status| status_block(transactions, *status)).collect();
    let texts = pack_blocks(&blocks, TEXT_BUDGET.saturating_sub(header.len() + footer.len()));
    let (active_texts, terminal_texts) = texts.split_at(Status::ACTIVE.len());

    let mut components = vec![text_display(header), separator()];
    components.extend(active_texts.iter().flat_map(|text| text_displays(text)));
    components.push(separator());
    components.extend(terminal_texts.iter().flat_map(|text| text_displays(text)));
    components.extend([separator(), text_display(footer)]);
    vec![container(Some(ACCENT_COLOR), components)]
}

#[cfg(test)]
mod tests {
    use super::{super::model::TransactionItem, *};
    use crate::{
        discord::components::{v2_text, v2_usage},
        util::format::V2_COMPONENT_LIMIT,
    };

    fn sample(index: usize, status: Status, items: usize) -> Transaction {
        Transaction {
            id: format!("tx{index}"),
            channel_id: "1".to_owned(),
            message_id: None,
            groupe: format!("Groupe {index} — Ballas"),
            kind: if index.is_multiple_of(2) { "ACHAT" } else { "VENTE" }.to_owned(),
            prix: 1500.0 * (index + 1) as f64,
            statut: status.as_str().to_owned(),
            created_by_id: "42".to_owned(),
            updated_at: DbTime::now(),
            items: (0..items)
                .map(|i| TransactionItem {
                    transaction_id: format!("tx{index}"),
                    nom: format!("objet {i}"),
                    quantite: 1,
                    prix_unitaire: None,
                })
                .collect(),
        }
    }

    #[test]
    fn dashboard_lists_by_status_with_totals() {
        let transactions = vec![
            sample(0, Status::Preparation, 2),
            sample(1, Status::EnAttenteRdv, 0),
            sample(2, Status::LivrePaye, 1),
        ];
        let components = build_dashboard(&transactions, 7);
        let text = v2_text(&components);
        assert!(text.starts_with("## 📊 Transactions"));
        assert!(text.contains("**2** transaction(s) en cours · **4\u{202f}500** engagé(s) · **7** au total"));
        assert!(text.contains("### 🔧 Préparation (1)\n- **Groupe 0 — Ballas** · Achat · 1\u{202f}500 · 2 objets"));
        assert!(text.contains("### 💰 Avance (en attente de paiement) (0)\n-# Aucune"));
        assert!(text.contains("### ✅ Livré et payé (1)\n- **Groupe 2 — Ballas** · Achat · 4\u{202f}500 · 1 objet"));
        assert!(matches!(components.as_slice(), [Component::Container(_)]));
    }

    #[test]
    fn dashboard_stays_within_v2_limits() {
        let transactions: Vec<Transaction> = (0..400)
            .map(|i| sample(i, Status::ACTIVE[i % 3], i % 4))
            .chain((0..30).map(|i| sample(1000 + i, Status::TERMINAL[i % 3], 1)))
            .collect();
        let components = build_dashboard(&transactions, 430);
        let (text_len, count) = v2_usage(&components);
        assert!(text_len <= V2_TEXT_LIMIT, "{text_len} octets");
        assert!(count <= V2_COMPONENT_LIMIT, "{count} composants");
        let text = v2_text(&components);
        assert!(text.contains("### 🔧 Préparation (134)"));
        assert!(text.contains("autres"));
        assert!(text.contains("### 🔒 Clôturée (10)"), "les blocs terminés restent visibles");
    }
}
