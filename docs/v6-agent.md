# lbm-agent — notes de conception

Complément de la phase 3 du plan (`docs/v6-architecture-plan.md`, branche `v6`). Ce document
décrit ce qui est construit, au fur et à mesure ; le plan reste la référence pour le périmètre.

## Principe : un seul fil de décision

L'agent remplace, côté résident, `MainService`, `DisplayChangeCoordinator`, `EngineController`
et le `LatestRequestGate` de #609. En C#, ces décisions étaient réparties entre plusieurs
chemins asynchrones qui capturaient chacun les zones de leur instant et se disputaient le
daemon : c'est ce qui a permis à un Start périmé d'accrocher un bureau disparu (#607).

L'agent les réunit dans un **réconciliateur sans E/S** (`lbm-agent/src/reconcile.rs`) :

- une entrée à la fois, traitée jusqu'au bout (`Input` : changement d'écran, événement du
  hook, Start/Stop/Refresh de l'utilisateur, réveil d'un minuteur) ;
- des effets en sortie, dans l'ordre (`Effect` : Start, Stop, SaveEnabled, SaveLayout,
  minuteur, processus vu) ;
- la mise en page lue et reconstruite par un trait (`World` : signature d'affichage,
  reconstruction, état de la mise en page, choix Enabled).

Le runtime possède l'horloge et les E/S, le réconciliateur possède les décisions. `Effect::Start`
ne porte pas de zones : le runtime les calcule depuis la mise en page courante au moment
d'envoyer. Un Start périmé ne peut plus exister, et le verrou de #609 devient inutile.

Les minuteurs sont des effets (`WakeAfter`) qui reviennent en entrées (`Wake`) : les tests
tournent sur une horloge virtuelle, sans délai réel.

## Règles reprises du C#

Les tests C# de `DisplayChangeCoordinator` et `EngineController` sont la spécification
(`lbm-agent/tests/reconcile.rs`, un test Rust par test C#) :

- **changement d'écran** : anti-rebond de 300 ms (une génération par notification, seule la
  dernière aboutit), stabilisation (relecture de la signature toutes les 100 ms jusqu'à deux
  lectures égales, 8 au plus), garde d'idempotence (une configuration identique à celle déjà
  construite ne reconstruit pas, #412, mais ré-accroche le hook, qui s'est décroché de
  lui-même) ; rien du tout pendant la veille ;
- **Refresh** (#443) : reconstruit sans anti-rebond ni garde, puis réaligne la garde ;
- **Start utilisateur** : enregistre Enabled avant d'accrocher (sauvegarde complète si
  l'éditeur a une géométrie non sauvegardée) ; une mise en page étrangère est simulée, jamais
  adoptée ; **Stop utilisateur** : enregistré, puis décroché dans tous les cas ;
- **ré-accrochage** : jamais pendant la veille, jamais contre un utilisateur qui a arrêté,
  jamais sur un moteur déjà accroché, jamais pendant que le chien de garde de reprise pilote ;
- **reprise après veille** : l'écran est d'abord stabilisé, puis le chien de garde réaffirme
  Start toutes les 500 ms jusqu'à trois Running consécutifs, six relances au plus (moteur
  légitimement en pause : jeu exclu au premier plan), soixante pas au plus ; une reprise plus
  récente remplace la précédente ;
- **Stopped juste après la connexion** : le hook n'a pas de mise en page, on la lui donne si
  l'utilisateur veut le moteur.

## Écarts assumés

- **#607 inclus** (comportement de #609) : une mise en page fraîche que l'utilisateur a
  arrêtée envoie Stop. Enabled est stocké par mise en page ; laisser le hook tranquille
  laisserait tourner la mise en page du bureau qui vient de disparaître. Le C# de `master`
  n'envoie rien.
- **Un seul Start par événement** : quand l'événement qui démarre le chien de garde vient
  d'envoyer un Start (ré-accrochage de l'écran stabilisé, remise d'une mise en page fraîche),
  le chien de garde laisse un pas à ce Start avant de vérifier. Le C# en envoie deux coup sur
  coup : sa première vérification passe avant que le Running du hook puisse arriver.
- **Chiens de garde successifs** : en C#, l'ancien chien de garde, en constatant qu'il est
  remplacé, remet le drapeau « actif » à faux alors que le nouveau tourne encore ; ici, le
  drapeau reste celui du plus récent.
- **Fin d'aperçu** : le C# laissait le hook sur la dernière géométrie prévisualisée, même
  jamais enregistrée (couper l'aperçu, annuler, quitter sans enregistrer). Ici un hook encore
  accroché revient à la mise en page courante, ou s'arrête si l'utilisateur ne la veut pas
  accrochée.
- **Secours pendant un aperçu** : le C# relançait le moteur sur la mise en page enregistrée
  sans regarder Enabled ; ici il n'est relancé que si l'utilisateur le veut accroché.

## Runtime (fait)
- `runtime::Agent` : la boucle d'événements. Chaque entrée (hook, sondage des écrans,
  minuteurs) passe par le réconciliateur ; ses effets sont exécutés dans l'ordre. Start
  calcule les zones à l'envoi et envoie Load+Run dans une seule trame `<Messages>`, comme le
  C# (le hook reste accroché d'une ré-application à l'autre).
- **Démarrage** (`Input::Boot`) : la première mise en page est construite, rien n'est
  envoyé ; le hook la reçoit quand il la demande (Connected puis Stopped). Un hook qui a
  survécu à l'agent précédent (D5) répond Running : il garde sa mise en page, rien n'est
  ré-accroché.
- `hook::HookClient` : une seule connexion pour les deux sens, maintenue ; commandes données
  hors connexion jetées (jamais rejouées périmées).
- `world::SystemWorld` : découverte (`lbm-display`), profil (`lbm-store`), zones.
- `watch` : la vérification du C# (signature sysfs, date des fichiers de sortie KWin/mutter),
  avancée par les événements : uevents DRM du noyau (netlink, sans privilège) et inotify sur
  le répertoire des fichiers de sortie (et celui de leur cible s'ils sont des liens). Tant
  qu'une source manque, sondage de 2 s comme le C# ; les deux en place, filet de 30 s. Un
  événement ne fait qu'avancer la vérification, qui décide toujours.
- `winwatch` (Windows) : le hook signale les changements d'écran, mais seulement quand il
  tourne ; un changement sans hook connecté laisserait la mise en page périmée, remise telle
  quelle au hook suivant. L'agent tient donc sa propre fenêtre cachée **top-level** (celle
  du hook, `WM_DISPLAYCHANGE` et `SPI_SETWORKAREA` n'atteignent pas une fenêtre message-only)
  et suit la session (WTS) : retour de l'écran de verrouillage ou du bureau sécurisé,
  reconnexion à la console. Chacun est un `DisplayChanged`, que l'anti-rebond fond avec le
  signalement du hook.
- `fake_hook` et `--fake-hook` : un hook qui n'accroche rien, sur un point de terminaison
  privé ; `--config-dir`/`--data-dir` isolent les profils. Tout essai hors session réelle
  doit passer les trois.
- `supervise::HookLauncher` (D5) : sur `Unreachable`, lance le hook posé à côté de l'agent
  (ou `--hook`), **détaché** (nouvelle session, `hook.log` à lui) pour qu'il survive à
  l'agent, avec `LBM_HOOK_UI=1` (le hook ne peut plus deviner d'après le chemin de son
  parent). Pas de doublon : ni pendant que le hook lancé démarre, ni si un autre hook de
  cet utilisateur tourne ; backoff pour un hook qui meurt en boucle. Testé au niveau
  processus avec le binaire de l'agent en faux hook (`--serve-fake-hook`) : lancé, il
  reçoit la mise en page ; l'agent tué, il tourne toujours.
- `gap_guard` (D7, `KScreenGapGuard`) : sous Plasma Wayland, quand le hook passe par le
  portail (pas par evdev), les sorties sont écartées d'un pixel logique pendant que le moteur
  tourne, pour que les barrières passent le validateur. Positions d'origine journalisées
  avant de toucher au compositeur (`kscreen-restore.json`, même forme que le C# : chacun
  récupère le journal de l'autre), restauration à l'arrêt, récupération au démarrage d'un
  journal laissé par un plantage ; une sortie déplacée par l'utilisateur entre-temps n'est pas
  touchée. Un Start qui déplace les sorties est abandonné : le changement d'écran qui suit
  reconstruit et envoie les zones dans la nouvelle géométrie. **Jamais en `--fake-hook`** : il
  déplacerait les vraies sorties.
- `api` (D6) : l'entrée des frontends. Trames à préfixe de longueur (celles du hook), JSON
  en PascalCase, socket 0600 `lbm-agent.sock` à côté du verrou d'instance (nom partagé dans
  `lbm_ipc::endpoint`). Version 1 : `Hello`, `Snapshot`, `Subscribe` (état puis un événement
  `State` à chaque changement), `Start`, `Stop`, `Refresh`, `Quit` (le hook d'abord : l'agent
  attend que le Quit soit écrit avant de partir). Méthode inconnue : erreur, jamais devinée.
  Version 2 : un abonné reçoit aussi chaque événement du hook tel quel (événement `Hook`,
  noms de `LittleBigMouseEvent`, `Connected`/`Dead` à la connexion et à la perte), ce que
  les suiveurs de l'UI lisent aujourd'hui (issue du Load, rapport de sonde, secours) ;
  `Probe` (le rapport revient en `Probed`) ; `SeenProcesses` (C# `ProcessesCollector` :
  processus vus au premier plan dans la session, chacun une fois, `Contains` compris).
  Version 3 : l'agent devient le seul écrivain. Un frontend envoie ce qu'il aurait
  enregistré, un `LayoutDocument` (`lbm-store` : options globales, document de la mise en
  page, modèles, liste d'exclusion — les formes du store), pour la mise en page qu'il édite
  (`LayoutId`, refusé si les écrans ont changé sous l'éditeur ou pour une mise en page
  étrangère). Appliqué à la copie de l'agent, il donne ce qu'un enregistrement puis un
  chargement auraient donné, et l'agent écrit ce que le frontend aurait écrit (testé octet
  pour octet sur le store). `SaveLayout` (le bouton Enregistrer, sans ré-envoi au hook,
  comme en C# où l'UI et le moteur partageaient le modèle), `Start` avec le document
  (« appliquer et démarrer » : appliqué, enregistré, démarré), `Preview` à chaque tick
  (Load+Run sans prologue ni écriture, jamais une mise en page identique à celle que le hook
  tient s'il tourne) et `EndPreview`, `SaveOptions` (C# `SaveLive`, plus le raccourci de
  secours dit au hook sous Windows, et `LoadAtStartup`, qui n'est pas une option stockée
  mais le démarrage avec la session lui-même : l'agent aligne l'entrée XDG ou la tâche
  planifiée). Le réconciliateur tient l'aperçu : Start ou Stop de
  l'utilisateur, reconstruction, perte du hook et secours y mettent fin ; un ré-accrochage
  pendant l'aperçu garde l'aperçu. `Previewing`, `Saved`, `LoadAtStartup` et `HideTrayIcon`
  dans l'état.
  Sous Windows (`winpipe`) : l'agent trouve le tube du hook de sa session
  (`LittleBigMouse-v1-session-{id}`, celui du client C#) et sert l'API sur le sien
  (`LittleBigMouse-Agent-v1-session-{id}`), sécurisé comme celui du hook : SYSTEM et
  l'utilisateur seuls, aucun client distant ni d'une autre session ; étiqueté intégrité
  moyenne quand l'agent est élevé, sans quoi un frontend non élevé ne pourrait pas écrire.
  Les tests de l'API passent sur les deux plateformes (socket, tube).
- Élévation au démarrage sous Windows (`elevation`, C# `WindowsElevation`, #512/#400) :
  avant même le verrou d'instance, si `StartElevated` est demandé (store, ou registre 5.x
  tant que l'import n'a pas eu lieu) **et** que l'utilisateur peut élever (jeton filtré
  d'administrateur), un seul consentement UAC relance l'agent élevé et celui-ci part sans
  prendre le verrou ; consentement refusé : il continue non élevé. Jamais d'invite pour un
  utilisateur standard, à qui elle demanderait des identifiants qu'il n'a pas. Les variables
  `LBM_*` sont passées en `--env:NOM=valeur` (ShellExecute donne un environnement neuf) et
  appliquées avant toute lecture. Le hook lancé par un agent élevé hérite de son élévation.
- Instance unique (`instance`, verrou `flock` / mutex nommé) prise avant tout, puis journal
  `agent.log` sur cinq générations (`log`) quand la sortie d'erreur n'est pas un terminal.
- Repli du Stop (C# : `StopCurrentSessionDaemons`) : un Stop émis sans connexion au hook
  termine le hook que cet agent a lancé (le noyau libère alors grab et barrières) ; jamais
  un autre, qui pourrait être celui de n'importe qui.
- Veille sous Linux (`sleep`) : jusqu'ici seul le hook Windows réagissait (écran éteint →
  `Suspended`, puis `Resumed`). logind annonce `PrepareForSleep(true)` ; l'agent tient un
  verrou d'inhibition *delay* tant que la machine est éveillée, ce qui lui laisse le temps
  d'envoyer Stop au hook (trame écrite) et de se mettre en `Suspended` avant de relâcher le
  verrou. Au réveil (`PrepareForSleep(false)`), `Resumed` : même chemin que sous Windows
  (stabilisation, puis chien de garde), et le verrou est repris. Rien n'est enregistré :
  l'utilisateur n'a rien arrêté. Sans bus système ni logind, la veille passe inaperçue,
  comme avant.
- Tray (`tray`, C# `TrayIconController`) : un frontend comme les autres, dans le processus.
  Il suit l'état de l'agent par l'API (`Subscribe`) et son menu envoie les mêmes requêtes
  (Ouvrir, Start, Stop, Rafraîchir, Quitter ; pas de mise à jour : le paquet de la
  distribution s'en charge). Le modèle (icône, état en mots, menu, requêtes) est commun ;
  `ksni` le dessine sous Linux (sans GTK), la zone de notification sous Windows
  (`Shell_NotifyIcon` : fenêtre cachée top-level sur son fil, clic gauche = Ouvrir, clic
  droit = menu, icône remise quand l'explorateur redémarre — `TaskbarCreated`). Icônes du
  C# (on / off / dead / paused, la veille de l'écran comprise), rendues une fois depuis ses
  SVG (`icons/render.sh`), prises à la taille des petites icônes du système sous Windows.
  Ouvrir lance le frontend donné par `--ui`, aucun par défaut tant que l'UI pilote encore
  le hook elle-même (phase 4). `--no-tray` pour une exécution sans tray. Sans hôte de tray,
  l'agent le dit et continue sans.
- Démarrage avec la session sous Linux (`autostart`, qui n'existait pas) : une entrée XDG
  autostart `littlebigmouse-agent.desktop` lançant l'agent, tenue par les crochets de
  persistance du C# (`IsAutostartScheduled` au chargement donne `LoadAtStartup`,
  `SetAutostart` à chaque enregistrement l'aligne). Spécification XDG suivie : l'entrée de
  l'utilisateur décide quand elle existe (éteinte par `Hidden=true` ou
  `X-GNOME-Autostart-enabled=false`), sinon une entrée système (paquet) ; l'éteindre retire
  celle de l'utilisateur, ou masque celle du système par `Hidden=true`. Avec `--config-dir`,
  l'entrée va sous ce répertoire, jamais dans celui de l'utilisateur.
- Sous Windows (`schtask`), la tâche planifiée du C# (`AutostartExtensions`), même nom
  (`LittleBigMouse_<domaine>_<utilisateur>`) : une tâche 5.x compte comme planifiée et le
  prochain enregistrement la réécrit sur l'agent (migration voulue par le plan ; sur une
  machine d'essai avec la 5.x, un agent de développement la reprend donc aussi, comme le
  faisait n'importe quel build C#). Enregistrée par `schtasks.exe` depuis une définition
  XML : ouverture de session de cet utilisateur, niveau demandé (élevée refusée : simple
  plutôt que rien), sans condition de batterie ni limite de durée, pas en session distante,
  et relance sur échec (D6). Jamais avec `--config-dir`.

- Fond d'écran (début) : `lbm_store::wallpaper_settings` lit et écrit `wallpaper.json` du
  plugin C# (une entrée par mise en page : mode, image « span », réglages par écran), au
  membre près — y compris les énumérations écrites en chaîne (`Mode`, `Style`) et celle
  écrite en nombre (`Kind`). Le découpage en millimètres est déjà porté
  (`lbm_layout::wallpaper`, phase 1). Restent le rendu des tranches et l'application
  (Plasma par zbus, `IDesktopWallpaper` sous Windows), puis la ré-application après
  reconstruction.

## Suite

Ce qui reste de la phase 3, par ordre de valeur :

1. **Checklist Windows sur machines réelles** (`docs/v6-windows-checklist.md`) : rien de ce
   qui suit ne remplace un passage à la main — dumps d'écran comparés au C#, import du
   registre, tubes et élévation, tray, tâche planifiée et sa migration, veille, dock/undock
   et #607, bureau sécurisé UAC.
2. **Fond d'écran « span »** : les réglages (`wallpaper_settings`) et le découpage
   (`lbm_layout::wallpaper`) sont là ; restent le rendu des tranches (fichiers adressés par
   contenu, comme `SpanRenderer`), l'application (Plasma par zbus au lieu de `busctl`,
   `IDesktopWallpaper` sous Windows) et la ré-application après reconstruction.
3. **Option « masquer l'icône »** suivie par le tray (l'état la publie déjà :
   `HideTrayIcon`).
4. **Exclusion par focus dans l'agent (D4)** : elle touche le hook (il perdrait
   `Excluded.txt` et la politique) — à faire avec l'allègement du hook, phase 5. L'agent
   tient déjà l'historique des processus vus, que le hook lui envoie.
5. `Current.xml` disparaît côté C# (phase 4 : l'arrêt utilisateur est déjà `Enabled=false`).
