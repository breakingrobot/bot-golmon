# Bot Discord — Transactions, Présences & Prospects

## Setup

```bash
pnpm install
cp .env.example .env   # renseigner DISCORD_TOKEN, DISCORD_CLIENT_ID, DISCORD_GUILD_IDS
pnpm db:migrate
pnpm deploy-commands    # enregistre les slash commands sur chaque guilde whitelistée
pnpm dev
```

## Multi-serveur (whitelist)

Le bot est multi-guild : chaque `Transaction`/`Presence`/`PresenceConfig` est scopée par `guildId`, aucune donnée n'est partagée entre serveurs.

`DISCORD_GUILD_IDS` (liste d'IDs séparés par des virgules) contrôle les serveurs autorisés :
- si renseigné : les commandes sont déployées uniquement sur ces guildes, et le bot **quitte automatiquement** tout serveur non listé (à l'ajout via `guildCreate`, et au démarrage via un balayage dans `ready`).
- si vide : aucune restriction (pratique en dev), déploiement global des commandes.

## Scripts

- `pnpm dev` — lance le bot en mode watch
- `pnpm build` / `pnpm start` — build puis lance en production
- `pnpm deploy-commands` — (re)déploie les slash commands
- `pnpm lint` / `pnpm lint:fix` — Oxc
- `pnpm db:migrate` / `pnpm db:studio` — Prisma

## Modules

### Présences

À configurer **une seule fois** ; ensuite plus rien n'est redemandé à la création :

```
/presence config role:@AOD absence_role:@Absent channel:#présences absence_channel:#absences
/presence create heure:21:00
```

- `/presence config` sans option affiche la configuration actuelle. La lecture est ouverte à tous, la modification demande « Gérer le serveur ».
- `/presence create` : seule `heure` est requise. `date` vaut aujourd'hui par défaut (bascule sur demain si l'heure est déjà passée), `titre` vaut « Présence ». Le rôle ciblé, le rôle d'exception et le salon viennent de la config.
- Boutons Présent / Absent / Retard (modal pour la durée) / Ping. L'embed affiche aussi les membres **sans réponse**.
- Rappels automatiques des non-répondants, au plus une fois toutes les 3h par présence ; clôture automatique à l'heure de l'événement (contrôle toutes les 5 min).
- `/presence stats [joueur]` : fiche individuelle (avec l'historique des changements d'avis) ou classement du serveur.
- `/presence forcevote utilisateur statut [presence] [retard]` : le staff saisit la réponse d'un membre à sa place. L'autocomplétion inclut les présences clôturées (🔒) pour corriger une statistique déjà comptabilisée.

### Absences

```
/absence declare motif:"Vacances" fin:20/08/2026
```

- `debut` vaut aujourd'hui par défaut ; une date couvre la journée entière (début 00:00, fin 23:59).
- Le **rôle d'absence** configuré est appliqué automatiquement, puis retiré à la fin de la période par le cron (contrôle toutes les 5 min). Si le membre a plusieurs absences qui se chevauchent, le rôle n'est retiré qu'à la fin de la dernière.
- Les présences ouvertes tombant dans la période passent automatiquement en **Absent**.
- L'absence est annoncée dans le salon configuré via `absence_channel`.
- `/absence end [membre]` met fin à une absence par anticipation, `/absence list` affiche les absences en cours.
- Le staff (Gérer les messages) peut déclarer ou clore l'absence d'un autre membre via l'option `membre`.

Si le bot ne peut pas appliquer le rôle (permission `Gérer les rôles` manquante, ou rôle d'absence situé au-dessus du sien dans la hiérarchie), l'absence est **quand même enregistrée** et le problème est signalé dans la réponse.

### Blocklist présences

```
/presence blocklist add membre:@Membre raison:"Inactif"
```

Les membres de la blocklist ne sont **jamais pingés** et sont marqués absents d'office à chaque nouvelle présence — indépendamment des absences ponctuelles. `/presence blocklist list` et `remove` complètent le groupe.

### Transactions

```
/transaction config dashboard_channel:#transactions
/transaction create groupe:X type:Vente prix:1500
```

- Tableau de bord permanent auto-actualisé dans le salon configuré : toutes les transactions groupées par statut, avec total engagé.
- L'embed détaillé de chaque transaction est posté dans le salon d'invocation, avec des **boutons de changement de statut** (le statut courant est désactivé).
- `/transaction add-item`, `status`, `view`, `cancel` avec autocomplétion.

### Véhicules

Registre des véhicules du groupe : quelle plaque appartient à quel membre Discord.

```
/vehicle config dashboard_channel:#véhicules
/vehicle add plaque:D6CS3P0G proprietaire:@Membre
/vehicle lookup plaque:D6CS3P0G
```

- Tableau de bord permanent auto-actualisé, **groupé par propriétaire**, plaques mises en évidence.
- `/vehicle add` récupère automatiquement le modèle via l'API GLife. Si l'API est injoignable ou ne connaît pas la plaque, l'ajout réussit quand même et le retour le précise.
- `/vehicle lookup` interroge l'API pour **n'importe quelle plaque**, enregistrée ou non : modèle, propriétaire en jeu, acheteur, statut illégal. Attention, le propriétaire renvoyé par l'API est celui **en jeu** (souvent une organisation) — distinct du propriétaire Discord du registre, affiché séparément.
- `/vehicle owner` transfère un véhicule, `/vehicle remove` le retire.
- Les plaques sont normalisées (majuscules, sans espaces ni tirets) : `d6cs 3p0g` et `D6CS3P0G` sont la même plaque, impossible de créer un doublon.

API utilisée : `GET https://api.glife.fr/roleplay/vehicles?plate=…` (publique, sans authentification). Les réponses sont mises en cache 5 minutes, avec un timeout de 8 s pour ne jamais bloquer une interaction Discord.

### Prospects

Avis hebdomadaires des gradés sur les membres en période d'essai, avec droit de véto des leads.

```
/prospect config role_prospect:@Prospect role_grade:@Gradé role_lead:@Lead categorie:#Essais salon:#avis
/prospect liste
/prospect avis membre:@Membre
/prospect veto poser membre:@Membre motif:Comportement inadapté
```

- **Un embed par semaine** dans le salon configuré, listant chaque prospect avec le décompte
  `✅ / ❌ / ⬜` et les motifs. Trois menus déroulants : donner son avis, poser un véto, consulter le détail.
- Le vote passe par une **modal** demandant le vote (Pour / Contre / Blanc) **et un motif obligatoire**.
  Un gradé n'a qu'un seul avis par prospect : revoter remplace le précédent.
- **Reset chaque lundi 00:00** (Europe/Paris) : la semaine écoulée est archivée en base et son embed
  reste visible dans le salon, un nouveau message est publié pour la semaine qui commence. Un
  rattrapage quotidien couvre le cas d'un bot hors ligne au moment de la bascule.
  `/prospect historique membre:@Membre` consulte les semaines passées.
- **Salon privé automatique** : dès qu'un membre reçoit le rôle prospect, un salon `essai-<pseudo>` est
  créé dans la catégorie configurée, visible uniquement par lui, les gradés et les leads.
  `/prospect salon membre:@Membre` fait la même chose manuellement. Perdre le rôle retire le membre de
  la liste de la semaine mais **conserve son salon**.
- **Véto** : réservé au rôle lead, motif obligatoire, affiché en rouge et bloquant.
  `/prospect veto lever` le retire. « Gérer le serveur » ne donne **pas** le droit de véto — seul le
  rôle lead configuré le donne ; cette permission ne sert qu'à `/prospect config` et `/prospect reset`.

Le bot doit disposer de la permission **Gérer les salons** pour créer les salons d'essai.

### Numéros

Annuaire des numéros de téléphone RP.

```
/numero config salon:#annuaire
/numero ajouter numero:5551234
/numero voir membre:@Membre
```

- Embed permanent auto-actualisé avec un bouton **« Ajouter / modifier mon numéro »** (ouvre une modal)
  et un bouton pour retirer le sien.
- Format imposé : `555` suivi de **4 à 6 chiffres** (`5551234`, `55512345`, `555123456`). La saisie est
  normalisée, donc `555-1234` et `5551234` sont équivalents.
- **Un numéro par membre** (en ajouter un nouveau remplace l'ancien) et un numéro ne peut pas être
  attribué à deux membres. Chacun gère le sien ; retirer celui d'un autre demande « Gérer les messages ».
- `/numero ajouter numero:5551234 membre:@Membre` inscrit **de force** le numéro d'un autre membre,
  en écrasant le sien sans confirmation. Réservé à « Gérer le serveur ».

## Fuseau horaire

Les saisies sont interprétées en **Europe/Paris** (`src/lib/datetime.ts`), indépendamment du fuseau du serveur d'hébergement — le conteneur de prod tourne en UTC. L'affichage utilise les timestamps Discord (`<t:…>`), donc chaque utilisateur voit l'heure dans son propre fuseau.
