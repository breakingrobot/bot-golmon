# Configurer les modules

Chaque module se configure **une fois par serveur** avec sa sous-commande `config` ; sans option, elle
affiche la configuration actuelle. La modifier demande **Gérer le serveur**. Les heures saisies sont en
**heure de Paris** ; les dates affichées s'adaptent au fuseau de chacun.

## Présences

```
/presence config role:@Membres absence_role:@Absent channel:#présences reminder_channel:#rappels absence_channel:#absences
```

| Option | Rôle |
|---|---|
| `role` | **Requis.** Membres concernés par les présences (pingés, comptés dans l'effectif). |
| `absence_role` | Rôle donné pendant une absence déclarée ; ses porteurs sont absents d'office et jamais pingés. |
| `channel` | Salon où sont publiées les présences (défaut : salon de la commande). |
| `reminder_channel` | Salon des rappels automatiques (défaut : salon de la présence). |
| `absence_channel` | Salon où sont annoncées les absences déclarées. |

Utilisation :

- `/presence create heure:21:00 [titre] [date:JJ/MM/AAAA] [description]` : la date vaut aujourd'hui par
  défaut, ou demain si l'heure est déjà passée. Le rôle est pingé, sauf les absents et la blocklist.
- Boutons **Présent / Absent / Retard** (durée demandée) **/ Ping**. L'embed liste les présents, absents,
  retards (🔄 = a changé d'avis) et les membres **sans réponse**.
- Automatique, toutes les 5 minutes : clôture à l'heure de l'événement ; rappel des non-répondants au
  plus toutes les 3 h ; pose et retrait du rôle d'absence.
- `/presence ping`, `close`, `changes` (historique des changements d'avis), `stats [joueur]` (fiche ou
  classement), `forcevote` (staff : saisir la réponse d'un membre, y compris sur une présence close 🔒),
  `cancel` (supprime une présence), `reset` (efface toutes les statistiques).
- **Blocklist** : `/presence blocklist add membre raison` : jamais pingé, absent d'office. `remove`, `list`.

## Absences

```
/absence declare motif:"Vacances" fin:20/08/2026 [debut:JJ/MM/AAAA] [membre:@…]
```

Une date couvre la journée entière. Le rôle d'absence est posé dès le début de la période et retiré à la
fin (ou à la fin de la dernière absence si plusieurs se chevauchent). Les présences ouvertes de la
période passent en **Absent**. `/absence end [membre]` termine une absence, `/absence list` les liste.
Déclarer ou clore l'absence d'un autre membre demande **Gérer les messages**.

> Si le rôle ne peut pas être posé (permission « Gérer les rôles » absente, ou rôle placé au-dessus de
> celui du bot), l'absence est quand même enregistrée et le problème est signalé.

## Transactions

```
/transaction config dashboard_channel:#transactions ping_role:@Logistique
```

- **Tableau de bord** permanent, mis à jour automatiquement : transactions en cours par statut, total
  engagé, dernières terminées. `clear_ping_role:true` retire le rôle mentionné à la création.
- `/transaction create groupe type:Vente prix:1500` publie la fiche dans le salon courant, avec boutons
  de statut, d'ajout d'objet et de suppression.
- `add-item`, `status`, `view`, `cancel` sont ouverts à tous ; `delete` demande **Gérer les messages**.
  Une transaction **clôturée** n'est plus modifiable.

## Véhicules

```
/vehicle config dashboard_channel:#véhicules
/vehicle add plaque:D6CS3P0G proprietaire:@Membre-ou-@Rôle [modele]
```

Tableau de bord groupé par propriétaire. Le modèle est récupéré via l'API GLife si non précisé (un échec
de l'API n'empêche pas l'ajout). `/vehicle lookup plaque` interroge l'API pour n'importe quelle plaque
(modèle, propriétaire **en jeu**, acheteur, illégal). `owner` transfère, `remove` retire. Les plaques sont
normalisées : `d6cs 3p0g` = `D6CS3P0G`.

## Recrues

```
/recrue config role_recrue:@Recrue role_grade:@Gradé role_lead:@Lead categorie:#Essais salon:#avis-recrues
```

| Option | Rôle |
|---|---|
| `role_recrue` | **Requis.** Rôle des membres en période d'essai : son attribution ajoute le membre au suivi. |
| `role_grade` | **Requis.** Rôle autorisé à donner son avis. |
| `role_lead` | Rôle autorisé à poser un véto (les leads votent aussi). |
| `categorie` | Catégorie où créer les salons d'essai privés. |
| `salon` | Salon où est publié le message d'avis de la semaine. |

- **Chaque semaine** (lundi 00:00), un nouveau message liste les recrues avec le décompte ✅ / ❌ / ⬜.
  Trois menus : donner son avis (vote **et motif obligatoires**, un avis par gradé, modifiable), poser
  un véto, voir le détail. Les semaines passées restent visibles et consultables via `/recrue historique`.
- Recevoir le rôle recrue crée un salon `essai-<pseudo>` visible par la recrue, les gradés et les leads.
  Perdre le rôle retire de la liste mais conserve le salon.
- `/recrue liste`, `avis`, `ajouter`, `retirer`, `salon`, `historique` (gradés) ; `veto poser` / `veto lever`
  (leads uniquement : « Gérer le serveur » ne donne **pas** le véto) ; `reset` force une nouvelle semaine.

## Numéros

```
/numero config salon:#annuaire
```

Annuaire permanent avec boutons « Ajouter / modifier mon numéro » et « Retirer mon numéro ». Format :
`555` + 4 à 6 chiffres (`555-1234` accepté). Un numéro par membre, unique sur le serveur.
`/numero ajouter numero [membre]` (pour un autre membre : **Gérer le serveur**), `voir`, `supprimer`, `liste`.
