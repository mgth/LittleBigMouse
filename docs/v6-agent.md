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
- `watch` : sondage Linux de 2 s (signature sysfs, fichiers de sortie KWin/mutter).
- `fake_hook` et `--fake-hook` : un hook qui n'accroche rien, sur un point de terminaison
  privé ; `--config-dir`/`--data-dir` isolent les profils. Tout essai hors session réelle
  doit passer les trois.

## Suite

1. Supervision du hook (D5) : lancement détaché qui survit à un plantage de l'agent,
   relance avec backoff sur `Unreachable`, instance unique, journaux.
2. `KScreenGapGuard` (D7) : prologue et épilogue autour du hook sous KWin.
3. Sources Linux : inotify et uevents DRM à la place du sondage, veille par
   `PrepareForSleep` de logind.
4. API du frontend (JSON, D6), tray, autostart ; Windows (point de terminaison par session,
   élévation, sources).
