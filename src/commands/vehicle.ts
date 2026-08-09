import { ChannelType, SlashCommandBuilder, type SlashCommandStringOption } from "discord.js";
import type { Command } from "../types/command.js";
import {
  autocompleteVehicle,
  handleVehicleAdd,
  handleVehicleConfig,
  handleVehicleLookup,
  handleVehicleOwner,
  handleVehicleRemove,
} from "../modules/vehicles/commands/index.js";

function registeredPlateOption(option: SlashCommandStringOption) {
  return option
    .setName("plaque")
    .setDescription("Plaque d'immatriculation")
    .setRequired(true)
    .setAutocomplete(true);
}

export const command: Command = {
  data: new SlashCommandBuilder()
    .setName("vehicle")
    .setDescription("Registre des véhicules du groupe")
    .addSubcommand((sub) =>
      sub
        .setName("add")
        .setDescription("Enregistre un véhicule (le modèle est récupéré automatiquement)")
        .addStringOption((option) =>
          option.setName("plaque").setDescription("Plaque d'immatriculation").setRequired(true),
        )
        .addUserOption((option) =>
          option.setName("proprietaire").setDescription("Propriétaire Discord").setRequired(true),
        )
        .addStringOption((option) =>
          option.setName("modele").setDescription("Modèle (sinon récupéré via l'API)"),
        ),
    )
    .addSubcommand((sub) =>
      sub
        .setName("lookup")
        .setDescription("Informations d'un véhicule à partir de sa plaque")
        .addStringOption((option) =>
          option.setName("plaque").setDescription("Plaque d'immatriculation").setRequired(true),
        ),
    )
    .addSubcommand((sub) =>
      sub
        .setName("owner")
        .setDescription("Change le propriétaire d'un véhicule enregistré")
        .addStringOption(registeredPlateOption)
        .addUserOption((option) =>
          option.setName("proprietaire").setDescription("Nouveau propriétaire").setRequired(true),
        ),
    )
    .addSubcommand((sub) =>
      sub
        .setName("remove")
        .setDescription("Retire un véhicule du registre")
        .addStringOption(registeredPlateOption),
    )
    .addSubcommand((sub) =>
      sub
        .setName("config")
        .setDescription("Configure le tableau de bord (sans option: affiche la config)")
        .addChannelOption((option) =>
          option
            .setName("dashboard_channel")
            .setDescription("Salon du tableau de bord des véhicules")
            .addChannelTypes(ChannelType.GuildText),
        ),
    ),

  async execute(interaction) {
    switch (interaction.options.getSubcommand()) {
      case "add":
        return handleVehicleAdd(interaction);
      case "lookup":
        return handleVehicleLookup(interaction);
      case "owner":
        return handleVehicleOwner(interaction);
      case "remove":
        return handleVehicleRemove(interaction);
      case "config":
        return handleVehicleConfig(interaction);
      default:
        return;
    }
  },

  async autocomplete(interaction) {
    return autocompleteVehicle(interaction);
  },
};
