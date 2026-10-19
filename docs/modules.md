# Configurer et utiliser les modules

## Les commandes

| Commande | Pour qui | Contenu |
|---|---|---|
| `/config <module>` | « Gérer le serveur » | Réglages de chaque module, **une fois par serveur**. Sans option, affiche la configuration actuelle. |
| `/gestion …` | Selon l'action (gradés par défaut) | Actions de staff : votes forcés, annulations, planning des présences, blocklist, panneau des absences… |
| `/presence`, `/absence`, `/transaction`, `/vehicle`, `/recrue`, `/numero`, `/cotisation`, `/contact`, `/coffre` | Selon l'action, voir [Permissions](#permissions) | Utilisation courante. |
| Clic droit → *Applications* | Voir [plus bas](#actions-au-clic-droit) | Raccourcis sur un membre ou un message. |

`/config` n'apparaît qu'aux membres ayant « Gérer le serveur ». Les autres commandes sont visibles de tous
et vérifient le niveau de chaque action à l'exécution ([Permissions](#permissions)).

Les heures saisies sont en **heure de Paris** ; les dates affichées s'adaptent au fuseau de chacun.

## Permissions

Chaque action du bot a un **niveau** : tout le monde, membres du groupe, gradés, leads ou « Gérer le
serveur ». Les niveaux s'emboîtent : un lead peut tout ce que peut un gradé, qui peut tout ce que peut un
membre.

| Niveau | Qui |
|---|---|
| Membres du groupe | Rôle réglé par `/config permission role_membre:@…`. Sans ce réglage, le niveau ne laisse passer que les gradés et au-dessus. |
| Gradés | Rôle gradé de `/config recrue`, ou permission Discord « Gérer les messages » sur le serveur (accordée dans un seul salon, elle ne compte pas). |
| Leads | Rôle lead de `/config recrue`. |
| « Gérer le serveur » | Permission Discord. Elle ouvre **toutes** les actions, sauf les actions *strictes*. |

Les actions **strictes** (suivi, avis et vétos des recrues) ne regardent que les rôles : ni « Gérer le
serveur » ni « Gérer les messages » n'y donnent accès.

```
/config permission                                         ← affiche tous les niveaux (✏️ = modifié)
/config permission role_membre:@Membres
/config permission action:contact.ajouter niveau:Tout le monde
/config permission action:coffre.voir niveau:Leads
/config permission action:coffre.voir niveau:Par défaut    ← revient au niveau par défaut
```

L'action se choisit dans la liste proposée par Discord. Changer un niveau demande « Gérer le serveur ».
Les pages du site suivent les mêmes réglages (`site.stats`, `cotisation.stats`, `coffre.voir`…).

| Action | Module | Ce qu'elle permet | Par défaut |
|---|---|---|---|
| `presence.creer` | Présences | Créer, relancer et clôturer une présence | Tout le monde |
| `presence.stats` | Présences | Statistiques et historique d'une présence | Tout le monde |
| `presence.gerer` | Présences | Forcer une réponse, annuler une présence, planning | Gradés |
| `presence.reset` | Présences | Réinitialiser les statistiques de présence | « Gérer le serveur » |
| `presence.blocklist` | Présences | Ajouter ou retirer un membre de la blocklist | Gradés |
| `absence.autrui` | Absences | Absence d'un autre membre, panneau des absences | Gradés |
| `transaction.utiliser` | Transactions | Créer, compléter, changer le statut, annuler | Tout le monde |
| `transaction.supprimer` | Transactions | Supprimer définitivement une transaction | Gradés |
| `transaction.historique` | Transactions | Historique et export sur le site | « Gérer le serveur » |
| `vehicule.utiliser` | Véhicules | Enregistrer, consulter, modifier, retirer un véhicule | Tout le monde |
| `numero.ajouter_autrui` | Numéros | Enregistrer le numéro d'un autre membre | « Gérer le serveur » |
| `numero.supprimer_autrui` | Numéros | Supprimer le numéro d'un autre membre | Gradés |
| `recrue.suivi` | Recrues | Liste, détail et historique des avis, ajout et retrait, salon d'essai *(stricte)* | Gradés |
| `recrue.avis` | Recrues | Donner un avis sur une recrue *(stricte)* | Gradés |
| `recrue.veto` | Recrues | Poser ou lever un véto *(stricte)* | Leads |
| `recrue.reset` | Recrues | Forcer une nouvelle semaine d'avis | « Gérer le serveur » |
| `cotisation.ajouter` | Cotisations | Déclarer ses cotisations | Tout le monde |
| `cotisation.gerer` | Cotisations | Cotisations des autres : déclarer, consulter, supprimer | Gradés |
| `cotisation.stats` | Cotisations | Statistiques (Discord et site) | Gradés |
| `coffre.voir` | Coffres | Stock et historique (Discord et site) | Gradés |
| `coffre.gerer` | Coffres | Relier un personnage, nommer une propriété | Gradés |
| `contact.voir` | Contacts | Consulter le répertoire | Tout le monde |
| `contact.ajouter` | Contacts | Ajouter un contact | Membres du groupe |
| `contact.modifier` | Contacts | Modifier ou supprimer un contact | Gradés |
| `contact.notes` | Contacts | Lire et écrire les notes du staff (Discord et site) | Gradés |
| `site.stats` | Site | Statistiques du serveur sur le site | « Gérer le serveur » |
| `site.journal` | Site | Journal du staff sur le site | « Gérer le serveur » |

Déclarer ses propres absences, répondre aux présences, enregistrer ou supprimer son propre numéro et
supprimer ses propres cotisations restent ouverts à tous.

## Présences

```
/config presence role:@Membres absence_role:@Absent channel:#présences reminder_channel:#rappels absence_channel:#absences rapport_salon:#staff-assiduité seuil_alerte:50
```

| Option | Rôle |
|---|---|
| `role` | **Requis.** Membres concernés par les présences (pingés, comptés dans l'effectif). |
| `absence_role` | Rôle donné pendant une absence déclarée ; ses porteurs sont absents d'office et jamais pingés. |
| `channel` | Salon où sont publiées les présences (défaut : salon de la commande). |
| `reminder_channel` | Salon des rappels automatiques (défaut : salon de la présence). |
| `absence_channel` | Salon où sont annoncées les absences déclarées. |
| `rapport_salon` | Salon du [rapport hebdomadaire](#rapport-hebdomadaire) d'assiduité (non défini : pas de rapport). |
| `seuil_alerte` | Taux de présence (1–100 %, **50** par défaut) sous lequel un membre est signalé dans le rapport. |
| `rapport_desactiver` | `true` arrête le rapport hebdomadaire. |

Utilisation :

- `/presence create heure:21:00 [titre] [date:JJ/MM/AAAA] [description]` : la date vaut aujourd'hui par
  défaut, ou demain si l'heure est déjà passée. Le rôle est pingé, sauf les absents et la blocklist.
- Boutons **Présent / Absent / Retard** (durée demandée) **/ Ping**. L'embed liste les présents, absents,
  retards (🔄 = a changé d'avis) et les membres **sans réponse**.
- Automatique, toutes les 5 minutes : clôture à l'heure de l'événement ; rappel des non-répondants au
  plus toutes les 3 h ; pose et retrait du rôle d'absence.
- `/presence ping`, `close`, `changes` (historique des changements d'avis), `stats [joueur]` (fiche ou
  classement).
- Staff : `/gestion presence forcevote` (saisir la réponse d'un membre, y compris sur une présence
  close 🔒), `annuler` (supprime une présence), `reset` (efface toutes les statistiques, « Gérer le
  serveur »).
- **Blocklist** : `/gestion blocklist ajouter membre raison` : jamais pingé, absent d'office. `retirer`, `liste`.

### Rapport hebdomadaire

Si `rapport_salon` est défini, le bot y publie **chaque lundi à 09:00** (heure de Paris) le bilan de la
semaine écoulée (lundi → dimanche), une seule fois par semaine ; s'il était hors ligne à 09:00, il
l'envoie dès son retour, le lundi même. Le rapport ne pingue personne. Il contient :

- les présences clôturées de la semaine (présents, absents, retards, sans réponse) ;
- le **taux de présence** de la semaine (présents ÷ attendus ; un retard ne compte pas comme présent,
  comme dans `/presence stats`) et son évolution en points par rapport à la semaine précédente ;
- le **top 3** des membres les plus présents ;
- **⚠️ Sous le seuil** : les membres du rôle (hors bots et blocklist) dont le taux sur les **4 dernières
  semaines** est inférieur à `seuil_alerte`. Ceux dont les absences déclarées couvrent plus de la moitié
  des présences de la période restent listés, marqués *(absence déclarée)*.

Les attendus d'une présence sont les membres actuels du rôle déjà arrivés sur le serveur à sa date, plus
ceux qui y ont répondu. Une semaine sans présence clôturée est signalée telle quelle ; s'il n'y en a eu
aucune depuis 4 semaines, le rapport n'est pas envoyé.

### Présences récurrentes

```
/gestion presence planifier jour:mercredi heure:21:00 [titre] [description] [publication:24]
```

La présence est publiée automatiquement chaque semaine, `publication` heures avant l'événement (24 par
défaut), dans le salon des présences, avec le ping habituel. `/gestion presence planning` liste les
plannings, `deplanifier` en supprime un (les présences déjà publiées restent).

## Absences

```
/absence declare motif:"Vacances" fin:20/08/2026 [debut:JJ/MM/AAAA] [membre:@…]
```

Une date couvre la journée entière. Le rôle d'absence est posé dès le début de la période et retiré à la
fin (ou à la fin de la dernière absence si plusieurs se chevauchent). Les présences ouvertes de la
période passent en **Absent**. Déclarer ou clore l'absence d'un autre membre : `absence.autrui` (gradés par défaut).

- `/absence end [membre]` termine l'absence en cours, ou **annule** la prochaine si elle n'a pas commencé.
- `/absence list` liste les absences en cours.
- L'annonce publiée dans le salon des absences est **supprimée automatiquement** quand l'absence se
  termine ou est annulée : le salon ne montre que les absences d'actualité.
- **Panneau** : `/gestion absence panneau [salon]` publie un message avec les boutons **Déclarer une
  absence** (formulaire : motif, dernier jour, premier jour) et **Terminer / annuler mon absence**. Plus
  simple que la commande pour la plupart des membres.

> Si le rôle ne peut pas être posé (permission « Gérer les rôles » absente, ou rôle placé au-dessus de
> celui du bot), l'absence est quand même enregistrée et le problème est signalé.

## Transactions

```
/config transaction dashboard_channel:#transactions ping_role:@Logistique
```

- **Tableau de bord** permanent, mis à jour automatiquement : transactions en cours par statut, total
  engagé, dernières terminées. `clear_ping_role:true` retire le rôle mentionné à la création.
- `/transaction create groupe type:Vente prix:1500` publie la fiche dans le salon courant, avec boutons
  de statut, d'ajout d'objet et de suppression.
- `add-item`, `status`, `view`, `cancel` sont ouverts à tous ; la suppression définitive se fait avec
  `/gestion transaction supprimer`.
  Une transaction **clôturée** n'est plus modifiable.
- **Historique** sur le site web (`/stats/<serveur>/transactions`) : filtres, totaux par mois, export
  CSV ([détails](site-web.md#transactions)).

## Véhicules

```
/config vehicule dashboard_channel:#véhicules
/vehicle add plaque:D6CS3P0G proprietaire:@Membre-ou-@Rôle [modele]
```

Tableau de bord groupé par propriétaire. Le modèle est récupéré via l'API GLife si non précisé (un échec
de l'API n'empêche pas l'ajout). `/vehicle lookup plaque` interroge l'API pour n'importe quelle plaque
(modèle, propriétaire **en jeu**, acheteur, illégal). `owner` transfère, `remove` retire. Les plaques sont
normalisées : `d6cs 3p0g` = `D6CS3P0G`.

## Recrues

```
/config recrue role_recrue:@Recrue role_grade:@Gradé role_lead:@Lead categorie:#Essais salon:#avis-recrues
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
- **Le dimanche à 18:00**, un rappel dans le salon des avis mentionne les gradés qui n'ont pas donné leur
  avis sur toutes les recrues de la semaine (une fois par semaine, rien si tout le monde a voté).
- Recevoir le rôle recrue crée un salon `essai-<pseudo>` visible par la recrue, les gradés et les leads.
  Perdre le rôle retire de la liste mais conserve le salon.
- `/recrue liste`, `avis`, `ajouter`, `retirer`, `salon`, `historique` (gradés) ; `veto poser` / `veto lever`
  (`recrue.veto`, leads par défaut : « Gérer le serveur » ne donne **pas** le véto) ; `/gestion recrue reset` force une nouvelle semaine.

## Numéros

```
/config numero salon:#annuaire
```

Annuaire permanent avec boutons « Ajouter / modifier mon numéro » et « Retirer mon numéro ». Format :
`555` + 4 à 6 chiffres (`555-1234` accepté). Un numéro par membre, unique sur le serveur.
`/numero ajouter numero [membre]` (pour un autre membre : `numero.ajouter_autrui`, « Gérer le serveur » par défaut), `voir`, `supprimer`, `liste`.

## Cotisations

Ce que chaque membre rapporte au groupe, en argent **propre** et/ou **sale**.

```
/cotisation ajouter description:"Vente de pièces" propre:25000 [sale:…] [date:JJ/MM/AAAA] [membre:@…]
```

| Commande | Pour qui | Effet |
|---|---|---|
| `ajouter` | Tout le monde | Déclare une cotisation : description obligatoire, au moins un montant (propre et/ou sale, 100 000 000 au plus chacun), date du jour par défaut (pas dans le futur). Déclarer pour un autre membre : gradés et leads. |
| `historique [membre] [jours]` | Tout le monde (soi) ; gradés et leads (les autres) | Dernières cotisations et totaux propre / sale. |
| `stats [jours] [membre]` | Gradés (`cotisation.stats`) | Totaux du groupe sur toute la période ou les N derniers jours, et classement des membres. |
| `supprimer` | Celui qui l'a déclarée, ou gradés et leads | Supprime une cotisation saisie par erreur (journalisé). |

Les gradés et leads sont ceux des rôles configurés avec `/config recrue`. `/config cotisation salon:#…`
annonce chaque nouvelle cotisation dans un salon ; `sans_annonce:true` arrête les annonces.

Sur le site : `/cotisations` (ou `/stats/<serveur>/cotisations`), voir [Site web](site-web.md#cotisations).

## Coffres

Le jeu publie chaque dépôt et retrait des coffres de propriété dans un salon Discord (webhook
« Coffre propriété » : personnage, propriété, objet, quantité, type, heure). Le bot lit ces messages et en
déduit le **stock** de chaque coffre et l'**historique** de chaque joueur.

**Mise en place**

1. Activer *Message Content Intent* dans le portail Discord (onglet **Bot**), puis redémarrer le bot : sans
   lui, Discord ne transmet pas le contenu des encadrés.
2. `/config coffre salon:#logs-coffre` : le salon est suivi, et son historique récent (jusqu'à 5 000
   messages) est importé tout de suite. Plusieurs salons possibles ; `retirer:true` arrête le suivi.
3. Facultatif : `/coffre nommer propriete:8740 nom:"Planque du port"` et
   `/coffre lier id_jeu:340772 membre:@…` (le numéro entre parenthèses après le nom du personnage).

Les messages publiés pendant que le bot était arrêté sont rattrapés à son redémarrage ; un message n'est
jamais compté deux fois.

| Commande | Effet |
|---|---|
| `/coffre stock [propriete]` | Stock par propriété et par objet. |
| `/coffre historique [propriete] [membre] [objet] [jours]` | Derniers dépôts et retraits (par membre : ses personnages reliés). |
| `/coffre lier id_jeu membre` | Relie un personnage du jeu à un membre Discord (journalisé). |
| `/coffre nommer propriete nom` | Nom lisible d'une propriété. |

Réservé aux gradés par défaut (`coffre.voir`, `coffre.gerer`). Le stock est calculé à partir des mouvements lus depuis
le début du suivi : un objet déjà présent avant peut apparaître en négatif après un retrait.

**Messages pris en compte** : seulement ceux du webhook du jeu. Le bot retient le webhook qui publie les
encadrés « Coffre » dans le salon (le plus fréquent au premier import) et ignore tous les autres webhooks et
applications, pour qu'un membre ne puisse pas imiter un encadré. Si le jeu change de webhook :
`/config coffre salon:#… retirer:true` puis `/config coffre salon:#…`. Une quantité supérieure à 1 000 000 est
ignorée.

Sur le site : `/stats/<serveur>/coffres`, voir [Site web](site-web.md#coffres).

## Contacts

Répertoire des autres groupes, entreprises et personnes du jeu, avec leur relation au groupe :
🟢 **allié**, ⚪ **neutre** ou 🔴 **hostile**. Chaque contact a une description publique et, en option, un
téléphone. Les membres du groupe ajoutent des contacts ; les gradés et leads les modifient et y ajoutent
des **notes réservées au staff**.

```
/contact ajouter nom:"Garage Benny's" type:Entreprise relation:Allié [telephone:555-0142] [description:…]
```

| Commande | Pour qui | Effet |
|---|---|---|
| `voir contact` | Tout le monde | Fiche du contact (type, relation, téléphone, description), en privé. Les gradés et leads y voient aussi les 10 dernières notes du staff (auteur et date). |
| `liste [relation] [type]` | Tout le monde | Tous les contacts, groupés par relation, en privé. |
| `ajouter nom type relation [telephone] [description]` | Membres du groupe (`contact.ajouter`) | Ajoute un contact. Nom unique sur le serveur (sans tenir compte des majuscules), 80 caractères au plus ; description 500 au plus. |
| `modifier contact [nom] [type] [relation] [telephone] [description]` | Gradés (`contact.modifier`) | Change les champs indiqués ; `-` efface le téléphone ou la description. |
| `supprimer contact` | Gradés (`contact.modifier`) | Supprime le contact et toutes ses notes. |
| `note contact texte` | Gradés (`contact.notes`) | Ajoute une note du staff (1000 caractères au plus), jamais montrée aux autres membres. |

Le contact se choisit dans la liste proposée par Discord (le nom exact fonctionne aussi). Ajouts,
modifications (avec l'ancienne et la nouvelle relation), suppressions et notes sont inscrits au
[journal du staff](#journal-du-staff).

`/config contact salon:#…` publie un **répertoire permanent** dans un salon, groupé par relation et mis à
jour à chaque changement ; il ne montre jamais les notes du staff. `sans_annuaire:true` le retire.

Sur le site : `/contacts` (ou `/stats/<serveur>/contacts`), avec les notes, voir
[Site web](site-web.md#contacts).

## Journal du staff

Les actions sensibles sont enregistrées : configuration modifiée (`/config`), votes forcés, présences
annulées ou planifiées, statistiques réinitialisées, blocklist, absences déclarées ou clôturées **pour un
autre membre**, transactions supprimées, recrues ajoutées ou retirées, vétos, numéros modifiés pour un
autre membre, cotisations déclarées pour un autre membre ou supprimées, contacts ajoutés, modifiés ou
supprimés et notes du staff sur les contacts.

- Sur le site : `/stats/<serveur>/journal`, filtrable par action (`site.journal`, « Gérer le serveur » par défaut).
- Sur Discord, en option : `/config journal salon:#journal-staff` publie chaque action dans ce salon
  (mentions affichées, jamais notifiées). `/config journal desactiver:true` arrête la publication.

## Actions au clic droit

Clic droit sur un membre ou un message → **Applications** :

| Action | Sur | Effet | Visible par défaut |
|---|---|---|---|
| Présences du membre | Membre | Sa fiche de présence (comme `/presence stats joueur`) | Tout le monde |
| Ajouter aux recrues | Membre | L'ajoute aux recrues de la semaine et crée son salon d'essai | `recrue.suivi` (gradés par défaut) |
| Ouvrir un ticket | Membre | Ouvre un ticket avec lui (comme `!newthread`) | « Gérer les messages » ; module ticket requis |
| Signaler au staff | Message | Transmet le message (texte, auteur, lien) dans le ticket de la personne qui signale, ouvert au besoin | Tout le monde ; module ticket requis |

## Statistiques sur le site

Les membres ayant `site.stats` (« Gérer le serveur » par défaut) retrouvent sur le site du bot (`/stats`, après
[connexion Discord](site-web.md#2-connexion-discord)) : assiduité de chaque membre aux présences, absences
en cours, recrues de la semaine avec leurs avis et vétos, transactions en cours, véhicules, et l'activité des
tickets si le module est configuré.
