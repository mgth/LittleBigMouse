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
  Windows (tube par session, DACL du hook) viendra avec l'agent Windows.
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

- Tray sous Linux (`tray`, `ksni`, sans GTK ; C# `TrayIconController`) : un frontend comme les
  autres, dans le processus. Il suit l'état de l'agent par l'API (`Subscribe`) et son menu
  envoie les mêmes requêtes (Ouvrir, Start, Stop, Rafraîchir, Quitter ; pas de mise à jour :
  le paquet de la distribution s'en charge). Icônes du C# (on / off / dead / paused, la
  veille de l'écran comprise), rendues une fois depuis ses SVG (`icons/render.sh`). Ouvrir
  lance le frontend donné par `--ui`, aucun par défaut tant que l'UI pilote encore le hook
  elle-même (phase 4). `--no-tray` pour une exécution sans tray. Sans hôte de tray
  (StatusNotifierWatcher absent), l'agent le dit et continue sans.

## Suite

1. API, suite : `SaveLayout`, `SaveOptions` (dont le raccourci de secours, que l'agent
   transmet au hook), `Preview`/`EndPreview` ; option « masquer l'icône » suivie par le
   tray ; autostart ; Windows (tray, points de terminaison par session, élévation, sources).
