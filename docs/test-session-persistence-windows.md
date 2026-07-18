# Session de test Windows — refactor persistance (moteur partagé + RegistryLayoutStore)

> Contexte : la persistance Windows (`PersistencyExtensions`, 535 lignes d'extensions registre)
> a été remplacée par le moteur partagé `LayoutPersistence` (LittleBigMouse.Plugins/Persistence)
> branché sur `RegistryLayoutStore`. Le mapping modèle↔DTO est désormais unique pour les deux OS.
>
> Déjà validé sous Linux : 46 tests verts (moteur complet sur store in-memory + format JSON),
> compilation de tous les projets, comportement Linux inchangé (DTOs déplacés verbatim).
>
> **Reste à valider : tout ce qui touche le runtime Windows réel** — le `RegistryLayoutStore`
> n'est pas testable sous Linux (API registre Windows-only), il n'est couvert que par relecture.

## 0. Préparation (avant de lancer quoi que ce soit)

- [ ] Sauvegarder le registre :
  `reg export "HKCU\SOFTWARE\Mgth\LittleBigMouse" %USERPROFILE%\lbm-backup.reg`
- [ ] Sauvegarder `%LOCALAPPDATA%\Mgth\LittleBigMouse\Excluded.txt` (et noter les autres fichiers du dossier)
- [ ] Noter l'état de la tâche planifiée : `schtasks /query /fo list | findstr LittleBigMouse`
- [ ] Compiler la solution complète sous Windows (le projet Server échouait sous Linux pour une
  raison d'environnement SDK — NETSDK1226 — vérifier qu'il compile normalement sous Windows)

Rollback à tout moment : fermer l'app, `reg import %USERPROFILE%\lbm-backup.reg`, restaurer Excluded.txt.

## 1. Compat utilisateur existant (le test le plus important)

Avec le registre actuel déjà peuplé :

- [ ] Lancer l'app : le layout est restauré à l'identique — positions mm (`XLocationInMm`/`YLocationInMm`),
  ratios (`PhysicalRatioX/Y`), `BorderResistance`, source active, moniteurs exclus
- [ ] Les options layout sont restaurées : Enabled, Algorithm, MaxTravelDistance, Freelook*, LoopX/Y,
  AdjustPointer/Speed, AllowOverlaps/Discontinuity
- [ ] Les options globales sont restaurées : Pinned, AutoUpdate, StartMinimized, StartElevated,
  DebugTools, ShowMonitorActionWarning, BorderValues, HideTrayIcon, DaemonPort
- [ ] **Lecture pure** : `reg export` avant/après lancement (sans sauvegarder) → diff vide, à une
  exception près : `ExcludedDefaultsVersion` peut être écrit une fois si absent (migration top-up).
  L'ancien code seedait le registre au chargement (`GetOrSet`) ; le nouveau ne doit RIEN écrire d'autre.
- [ ] Le moteur démarre et la souris traverse normalement (sanity E2E daemon)

## 2. Sauvegarde — format byte-for-byte

- [ ] Déplacer un moniteur, sauvegarder, inspecter dans regedit :
  valeurs `REG_SZ`, décimales avec point (culture invariante), bools en `"1"`/`"0"`
- [ ] Diff `reg export` avant/après sauvegarde : uniquement les changements attendus
- [ ] Redémarrer l'app : tout est relu correctement (round-trip complet)

## 3. Fix Priority (bug corrigé au passage — vérifier qu'il est bien corrigé)

Avant : `Priority` était sauvée en clé racine mais écrasée au chargement par la valeur
seedée `"Normal"` de la clé layout → ne persistait jamais.

- [ ] Mettre Priority ≠ Normal dans l'UI, sauvegarder, redémarrer → la valeur est conservée
- [ ] Vérifier dans regedit que `Layouts\{id}\Priority` est maintenant écrite à la sauvegarde

## 4. Install vierge (première exécution)

Renommer `HKCU\SOFTWARE\Mgth\LittleBigMouse` (ex. suffixe `.bak`) et déplacer le dossier
`%LOCALAPPDATA%\Mgth\LittleBigMouse` :

- [ ] L'app démarre avec les défauts, sans erreur
- [ ] `Excluded.txt` est créé et contient les 4 défauts (Epic, steamapps, Riot, XboxGames)
- [ ] Le registre reste (quasi) vide tant qu'on ne sauvegarde pas — pas de seeding au chargement
- [ ] Les moniteurs sont auto-placés depuis la config système (`Placed` faux → placement auto) ;
  **changement assumé** : au 2ᵉ lancement sans sauvegarde, ils sont TOUJOURS auto-placés
  (l'ancien seeding les figeait dès le 2ᵉ lancement — la nouvelle sémantique est celle de Linux)
- [ ] Sauvegarder → la clé se peuple entièrement ; relancer → tout est restauré

## 5. Migrations legacy (configs anciennes)

Simuler un vieux profil dans regedit :

- [ ] Supprimer `Priority`, `Pinned`, `AutoUpdate`, `StartMinimized`, `StartElevated`, `HomeCinema`
  de la clé racine et les créer dans `Layouts\{id}` → au chargement les valeurs sont reprises ;
  après sauvegarde elles arrivent en clé racine
- [ ] Renommer `ShowMonitorActionWarning` en `ShowAttachDetachWarning` (ancien nom) → la valeur
  est relue ; après sauvegarde le nouveau nom est écrit

## 6. Migration top-up des exclusions (#494)

- [ ] Écrire dans `Excluded.txt` uniquement les 3 défauts legacy (Epic, steamapps, Riot),
  supprimer la valeur `ExcludedDefaultsVersion` → lancement : `\XboxGames\` est ajouté au fichier,
  `ExcludedDefaultsVersion=1` écrit en clé racine
- [ ] Liste personnalisée (retirer un défaut legacy) → rien n'est ajouté, la version est écrite quand même
- [ ] Relancer → la migration ne retourne pas (une seule fois par version)

## 7. Autostart / SaveEnabled / élévation

- [ ] Cocher LoadAtStartup + sauvegarder → tâche planifiée créée ; décocher → supprimée
- [ ] Variante StartElevated : la tâche est créée en RunLevel Highest
- [ ] **Scénario élévation au chargement** (garde `IsLoading`) : LoadAtStartup + StartElevated actifs,
  redémarrer l'app → la tâche reste élevée (le chargement des options ne doit pas re-planifier
  la tâche à mi-parcours et perdre l'élévation)
- [ ] Toggle moteur on/off sans sauvegarder (chemin `SaveEnabled`) : `Layouts\{id}\Enabled` mis à jour,
  **les autres valeurs de la clé layout sont intactes** (c'est désormais un read-modify-write du
  document complet — vérifier qu'aucune valeur n'est perdue ni altérée), tâche planifiée alignée

## 8. Bordures par-moniteur (la parité du PR #503 — raison d'être du refactor)

- [ ] Mode PerModel : sauvegarder → PAS de sous-clé `Borders` sous `PhysicalMonitors\{id}`
- [ ] Passer en PerMonitor sans redémarrer : les bordures partent des valeurs modèle courantes
- [ ] Éditer les bordures d'un seul moniteur, sauvegarder → `Borders\Left/Top/Right/Bottom` écrites
  pour ce moniteur uniquement
- [ ] Redémarrer → `BordersCustomized` restauré, valeurs correctes, l'autre moniteur suit toujours son modèle
- [ ] Sauvegarder en mode PerModel après coup → les Borders par-moniteur survivent
- [ ] Bordures modèle (`monitors\{pnp}\Borders` + `Size\Width/Height`) : édition, sauvegarde, relecture OK

## 9. Sources / attach-detach

- [ ] `ActiveSource`, `DisplayName`, `Primary` écrits sous `PhysicalMonitors\{id}\{sourceId}\`
- [ ] Détacher un moniteur du bureau, le rattacher → sa géométrie pixel est restaurée depuis le store
- [ ] **Changement assumé** : la géométrie n'est plus sauvée au chargement (seulement à la sauvegarde) ;
  un détachement AVANT toute première sauvegarde n'a donc rien à restaurer — comportement Linux, rare

## 10. SaveLive (options appliquées immédiatement)

- [ ] Changer StartMinimized / AutoUpdate / Pinned / DebugTools sans bouton save → écrits immédiatement
  en clé racine ; effectifs au prochain démarrage
- [ ] **Changement assumé** : SaveLive écrit désormais TOUTES les options globales (comportement Linux),
  pas seulement les 6 historiques — vérifier qu'aucun effet de bord visible (ex. une valeur éditée
  mais non sauvée type HideTrayIcon devient persistée)
- [ ] La boucle live-save ne réagit pas pendant un chargement (pas d'écho d'écritures au lancement)

## 11. Multi-layout & divers

- [ ] Changer la topologie (débrancher/rebrancher un écran, dock) → chaque combinaison garde sa propre
  clé `Layouts\{id}` avec ses valeurs, sans contamination croisée
- [ ] `DaemonPort` personnalisé dans le registre → toujours honoré (il n'est plus seedé au chargement,
  mais il est maintenant écrit à la sauvegarde)
- [ ] Lancement en admin → `Elevated` détecté, UI cohérente
- [ ] Exclusion process : lancer un jeu sous un chemin exclu → LBM ne traverse pas (le daemon lit
  toujours le même `Excluded.txt`, chemin inchangé)

## Changements de comportement ASSUMÉS (ne pas les signaler comme bugs)

1. Plus de seeding du registre au chargement — les valeurs apparaissent à la première sauvegarde
2. `Placed` ne devient vrai qu'après une vraie sauvegarde (install vierge : auto-placement à chaque
   lancement tant qu'on n'a pas sauvé)
3. `SaveLive` écrit toutes les options globales, pas seulement les 6 historiques
4. `SaveEnabled` réécrit le document layout complet (valeurs identiques) au lieu de la seule valeur `Enabled`
5. `HomeCinema` n'est plus écrit dans la clé layout (global uniquement ; l'ancienne valeur layout
   reste lue en fallback)
6. `Priority`/`PriorityUnhooked` sont maintenant écrites aussi dans la clé layout (c'est le fix du §3)
7. La géométrie des sources attachées n'est plus sauvée au chargement (seulement à la sauvegarde)
