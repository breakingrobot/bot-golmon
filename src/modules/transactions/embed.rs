use super::model::{Status, Transaction, type_label};
use crate::{
    discord::{
        components::{ButtonExt as _, button, row},
        embed::{EmbedBuilder, footer, now, timestamp},
    },
    util::{
        format::{FIELD_LIMIT, field, format_price, len16, pack_fields, truncate_field},
        ids::encode_custom_id,
    },
};
use twilight_model::channel::message::{Component, Embed, component::ButtonStyle, embed::EmbedField};

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

/// Titre, pied de page et marges de l'API : on ne remplit pas les 6000 caractères.
const FIELD_BUDGET: usize = 5200;
/// Un champ est réservé au résumé « … » en cas de débordement.
const MAX_FIELDS: usize = 24;

fn format_line(transaction: &Transaction) -> String {
    let count = transaction.items.len();
    let items =
        if count > 0 { format!(" — {count} objet{}", if count > 1 { "s" } else { "" }) } else { String::new() };
    format!(
        "• **{}** · {} · {}{items}",
        transaction.groupe,
        type_label(&transaction.kind),
        format_price(transaction.prix)
    )
}

/// Regroupe les lignes en champs d'au plus 1024 caractères (« (suite) » ensuite).
fn chunked_fields(name: &str, lines: &[String], inline: bool) -> Vec<EmbedField> {
    if lines.is_empty() {
        return vec![field(name, "Aucune", inline)];
    }
    let mut fields: Vec<EmbedField> = Vec::new();
    for line in lines {
        match fields.last_mut() {
            Some(current) if len16(&current.value) + len16(line) < FIELD_LIMIT => {
                current.value.push('\n');
                current.value.push_str(line);
            }
            Some(_) => fields.push(field(format!("{name} (suite)"), line.clone(), inline)),
            None => fields.push(field(name, line.clone(), inline)),
        }
    }
    fields
}

/// Tableau de bord en un seul embed borné. `total` compte toutes les
/// transactions de la guilde, y compris celles écartées de la liste.
pub fn build_dashboard(transactions: &[Transaction], total: i64) -> Embed {
    let bucket =
        |status: Status| -> Vec<&Transaction> { transactions.iter().filter(|t| t.status() == status).collect() };
    let status_fields = |statuses: &[Status], inline: bool| -> Vec<EmbedField> {
        statuses
            .iter()
            .flat_map(|status| {
                let items = bucket(*status);
                let name = format!("{} {} ({})", status.emoji(), status.label(), items.len());
                let lines: Vec<String> = items.iter().map(|t| format_line(t)).collect();
                chunked_fields(&name, &lines, inline)
            })
            .collect()
    };

    let mut fields = status_fields(&Status::ACTIVE, false);
    fields.extend(status_fields(&Status::TERMINAL, true));
    let (kept, dropped) = pack_fields(fields, FIELD_BUDGET, MAX_FIELDS);

    let active: Vec<&Transaction> = transactions.iter().filter(|t| Status::ACTIVE.contains(&t.status())).collect();
    let active_value: f64 = active.iter().map(|t| t.prix).sum();

    let mut builder = EmbedBuilder::new().title("📊 Transactions").color(0x5865f2);
    for field in kept {
        builder = builder.field(field);
    }
    if !dropped.is_empty() {
        builder = builder.field(field(
            "…",
            format!("{} bloc(s) non affiché(s), faute de place. Utilise `/transaction view`.", dropped.len()),
            false,
        ));
    }
    builder
        .footer(footer(format!(
            "{} transaction(s) en cours · {} engagé(s) · {total} au total",
            active.len(),
            format_price(active_value)
        )))
        .timestamp(now())
        .build()
}
