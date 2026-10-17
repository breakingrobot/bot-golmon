use super::{api::ApiVehicle, service::Vehicle};
use crate::{
    discord::{
        components::{container, separator, text_display, text_displays},
        embed::{EmbedBuilder, footer},
    },
    util::{
        format::{V2_TEXT_LIMIT, and_others, bounded_lines, field, owner_mention, plural},
        time::DbTime,
    },
};
use twilight_model::channel::message::{Component, Embed};

/// Titre, résumé et pied de page sont décomptés à part ; on garde une marge.
const TEXT_BUDGET: usize = V2_TEXT_LIMIT - 100;
/// Une organisation peut posséder des dizaines de véhicules : sa liste est
/// bornée pour laisser de la place aux autres propriétaires.
const OWNER_BUDGET: usize = 1000;
const ACCENT_COLOR: u32 = 0x5865f2;

fn vehicle_line(vehicle: &Vehicle) -> String {
    match &vehicle.modele {
        Some(modele) => format!("- `{}` — {modele}", vehicle.plate),
        None => format!("- `{}`", vehicle.plate),
    }
}

/// Registre en Components V2, groupé par propriétaire, les mieux fournis en premier.
pub fn build_dashboard(vehicles: &[Vehicle]) -> Vec<Component> {
    let footer = format!("-# Mis à jour <t:{}:R>", DbTime::now().unix());
    if vehicles.is_empty() {
        return vec![container(
            Some(ACCENT_COLOR),
            vec![
                text_display("## 🚗 Véhicules\nAucun véhicule enregistré. Ajoute-en un avec `/vehicle add`."),
                separator(),
                text_display(footer),
            ],
        )];
    }

    let mut owners: Vec<(&str, Vec<&Vehicle>)> = Vec::new();
    for vehicle in vehicles {
        match owners.iter_mut().find(|(owner, _)| *owner == vehicle.owner_id) {
            Some((_, owned)) => owned.push(vehicle),
            None => owners.push((&vehicle.owner_id, vec![vehicle])),
        }
    }
    owners.sort_by_key(|(_, owned)| std::cmp::Reverse(owned.len()));

    let blocks: Vec<String> = owners
        .iter()
        .enumerate()
        .map(|(index, (owner, owned))| {
            let lines: Vec<String> = owned.iter().map(|v| vehicle_line(v)).collect();
            format!(
                "{}{} — {} véhicule{}\n{}",
                // Ligne vide entre deux propriétaires.
                if index > 0 { "\n" } else { "" },
                owner_mention(owner, owned[0].owner_is_role),
                owned.len(),
                plural(owned.len()),
                bounded_lines(&lines, OWNER_BUDGET, and_others)
            )
        })
        .collect();

    let header = format!("## 🚗 Véhicules\n**{}** véhicule(s) · **{}** propriétaire(s)", vehicles.len(), owners.len());
    let list = bounded_lines(&blocks, TEXT_BUDGET.saturating_sub(header.len() + footer.len()), |hidden| {
        let count: usize = owners[owners.len() - hidden..].iter().map(|(_, owned)| owned.len()).sum();
        format!("\n… et {hidden} autre{s} propriétaire{s}, {count} véhicule{}", plural(count), s = plural(hidden))
    });

    let mut components = vec![text_display(header), separator()];
    components.extend(text_displays(&list));
    components.extend([separator(), text_display(footer)]);
    vec![container(Some(ACCENT_COLOR), components)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        discord::components::{v2_text, v2_usage},
        util::format::V2_COMPONENT_LIMIT,
    };

    fn vehicle(plate: &str, owner: &str, is_role: bool, modele: Option<&str>) -> Vehicle {
        Vehicle {
            plate: plate.to_owned(),
            owner_id: owner.to_owned(),
            owner_is_role: is_role,
            modele: modele.map(str::to_owned),
        }
    }

    #[test]
    fn dashboard_groups_by_owner() {
        let vehicles = vec![
            vehicle("AAA111", "100", false, Some("Sultan")),
            vehicle("BBB222", "200", true, None),
            vehicle("CCC333", "200", true, Some("Kuruma")),
        ];
        let components = build_dashboard(&vehicles);
        let text = v2_text(&components);
        assert!(text.starts_with("## 🚗 Véhicules\n**3** véhicule(s) · **2** propriétaire(s)"));
        assert!(text.contains(
            "<@&200> — 2 véhicules\n- `BBB222`\n- `CCC333` — Kuruma\n\n<@100> — 1 véhicule\n- `AAA111` — Sultan"
        ));
        assert!(matches!(components.as_slice(), [Component::Container(_)]));
        assert!(v2_text(&build_dashboard(&[])).contains("Aucun véhicule enregistré"));
    }

    #[test]
    fn dashboard_stays_within_v2_limits() {
        let mut vehicles: Vec<Vehicle> = (0..150)
            .map(|i| vehicle(&format!("ORG{i:03}"), "999999999999999999", true, Some("Véhicule de société")))
            .collect();
        vehicles.extend((0..300u64).map(|i| {
            vehicle(&format!("P{i:05}"), &(100_000_000_000_000_000 + i / 2).to_string(), false, Some("Élégie"))
        }));
        let components = build_dashboard(&vehicles);
        let (text_len, count) = v2_usage(&components);
        assert!(text_len <= V2_TEXT_LIMIT, "{text_len} octets");
        assert!(count <= V2_COMPONENT_LIMIT, "{count} composants");
        let text = v2_text(&components);
        assert!(text.contains("<@&999999999999999999> — 150 véhicules"));
        assert!(text.contains("autres propriétaires, "));
    }
}
