use super::{api::ApiVehicle, service::Vehicle};
use crate::{
    discord::embed::{EmbedBuilder, footer, now},
    util::format::{field, owner_mention, pack_fields, truncate_field},
};
use twilight_model::channel::message::Embed;

const FIELD_BUDGET: usize = 5200;
const MAX_OWNER_FIELDS: usize = 24;

fn vehicle_line(vehicle: &Vehicle) -> String {
    match &vehicle.modele {
        Some(modele) => format!("`{}` — {modele}", vehicle.plate),
        None => format!("`{}`", vehicle.plate),
    }
}

/// Registre groupé par propriétaire, les mieux fournis en premier.
pub fn build_dashboard(vehicles: &[Vehicle]) -> Embed {
    let builder = EmbedBuilder::new().title("🚗 Véhicules").color(0x5865f2).timestamp(now());
    if vehicles.is_empty() {
        return builder.description("Aucun véhicule enregistré. Ajoute-en un avec `/vehicle add`.").build();
    }

    let mut owners: Vec<(&str, Vec<&Vehicle>)> = Vec::new();
    for vehicle in vehicles {
        match owners.iter_mut().find(|(owner, _)| *owner == vehicle.owner_id) {
            Some((_, owned)) => owned.push(vehicle),
            None => owners.push((&vehicle.owner_id, vec![vehicle])),
        }
    }
    owners.sort_by_key(|(_, owned)| std::cmp::Reverse(owned.len()));

    let fields = owners
        .iter()
        .map(|(owner, owned)| {
            let lines = owned.iter().map(|v| vehicle_line(v)).collect::<Vec<_>>().join("\n");
            field(
                format!("{} véhicule{}", owned.len(), if owned.len() > 1 { "s" } else { "" }),
                truncate_field(&format!("{}\n{lines}", owner_mention(owner, owned[0].owner_is_role))),
                true,
            )
        })
        .collect();

    // Borner le nombre de champs ne suffit pas : 24 champs pleins font quatre
    // fois la limite de 6000 caractères d'un embed.
    let (kept, dropped) = pack_fields(fields, FIELD_BUDGET, MAX_OWNER_FIELDS);
    let kept_count = kept.len();
    let mut builder = builder;
    for f in kept {
        builder = builder.field(f);
    }
    if !dropped.is_empty() {
        let hidden: usize = owners[kept_count..].iter().map(|(_, owned)| owned.len()).sum();
        builder = builder.field(field(
            "…",
            format!("+{} autre(s) propriétaire(s), {hidden} véhicule(s)", dropped.len()),
            false,
        ));
    }
    builder.footer(footer(format!("{} véhicule(s) · {} propriétaire(s)", vehicles.len(), owners.len()))).build()
}

pub fn build_lookup(api: &ApiVehicle, registered: Option<&Vehicle>) -> Embed {
    let mut builder = EmbedBuilder::new()
        .title(format!("🚗 {}", api.name.as_deref().unwrap_or("Véhicule")))
        .color(if api.illegal { 0xe74c3c } else { 0x2ecc71 })
        .field(field("Plaque", format!("`{}`", api.plate), true))
        .field(field(
            "Modèle",
            api.name.clone().or_else(|| api.model.clone()).unwrap_or_else(|| "Inconnu".to_owned()),
            true,
        ))
        .field(field("Illégal", if api.illegal { "⚠️ Oui" } else { "Non" }, true));

    // `owner` vient du jeu (joueur ou organisation) : à ne pas confondre avec le
    // propriétaire Discord enregistré dans le bot, affiché juste en dessous.
    if let Some(owner) = &api.owner {
        builder = builder.field(field("Propriétaire en jeu", owner.name.clone(), true));
    }
    if let Some(buyer) = &api.buyer {
        builder = builder.field(field("Acheteur", buyer.name.clone(), true));
    }
    builder = builder.field(field(
        "Dans le registre",
        match registered {
            Some(v) => format!(
                "{}{}",
                owner_mention(&v.owner_id, v.owner_is_role),
                v.modele.as_deref().map(|m| format!(" — {m}")).unwrap_or_default()
            ),
            None => "Non enregistré · `/vehicle add` pour l'ajouter".to_owned(),
        },
        false,
    ));
    if let Some(model) = &api.model {
        builder = builder.footer(footer(format!("Code modèle : {model}")));
    }
    builder.build()
}
