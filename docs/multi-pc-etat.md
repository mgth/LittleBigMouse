# Mode multi-PC : état du travail et reste à faire

État au 2026-11-24, branche `v6` (`cf2bc5c7`, suivi jusqu’à `d7204166`), après la fusion des PR #800 (le mode)
et #801 (fonds d'écran suivis en direct).

Mis à jour au fil des travaux (dernière fois le 2026-11-24, maître Windows) : voir « Travaux en cours » à la fin.

Ce document dit ce qui est livré, ce qui a réellement été essayé, et ce qu'il manque
pour que n'importe quelle paire Linux/Windows fonctionne dans les deux sens. Le
contrat technique (protocole, modèle, règles de sûreté) est dans
[`multi-pc.md`](multi-pc.md) ; il n'est pas répété ici.

## Ce que fait le mode

Une souris et un clavier, sur le **maître**, pilotent les écrans d'une autre machine,
l'**esclave**, à la manière de Synergy/Deskflow/Barrier. La différence avec ces outils :
l'esclave publie ses écrans (EDID, taille physique, position) et le maître les place
dans son layout en millimètres, à côté des siens. C'est le moteur habituel de
LittleBigMouse qui route le pointeur sur l'ensemble ; l'esclave ne fait qu'injecter.

Un écran connu des deux machines (KVM, ou moniteur câblé sur deux entrées) est **un seul
moniteur à deux sources**, à condition que les deux machines lui lisent la même clé de
dalle (constructeur, produit et numéro de série de l'EDID) et que cette clé soit unique
parmi les écrans. Sans numéro de série exploitable, ou derrière un KVM qui masque
l'EDID, ce sont deux moniteurs distincts ; avec deux dalles de même clé, la source
distante est rattachée à la mauvaise. Sa source active dit quelle machine il affiche, et donc à
qui va le pointeur quand il y entre.

## Ce qui est livré

| Brique | Où | Contenu |
|---|---|---|
| Réseau | `lbm-net` | TCP 24817, TLS 1.3 mutuel, certificats auto-signés épinglés, appairage confirmé des deux côtés, `peers.json`, keepalive |
| Flux d'entrée | `lbm-ipc/src/input.rs` | Trames binaires `Enter/Leave/Motion/Button/Wheel/Key/Locks`, codes evdev, positions absolues en pixels du receveur ; `Locks` retiré pour un pair qui n'annonce pas la capacité `InputLocks` |
| Modèle | `lbm-layout`, `lbm-display`, `lbm-store`, `lbm-zones` | Machine portée par la source, clé de dalle inter-machines, fusion des écrans distants, zones avec hôte |
| Agent | `lbm-agent/src/peers` | Gestion des pairs, API (`Peers`, `SetMultiPc`, `PeerConnect`…), relais du flux, un esclave « sélectionné » |
| Hook maître | `lbm-hook/src/hook/crossing` (règles, toutes plateformes), `lbm-hook/src/hook/linux/evdev` et le maître Windows (ligne suivante) | Traversée, capture des claviers quand le pointeur est parti, retour local si le hook maître perd son relais, curseur local masqué |
| Maître Windows (PR #828) | `lbm-hook/src/hook/ll_master.rs` (neutre, testé partout avec le vrai moteur), `hook/windows/master.rs`, `remote::keymap::linux_key` | Hooks bas niveau `WH_MOUSE_LL` et `WH_KEYBOARD_LL` sur l'automate commun : mouvement avalé et lu comme déplacement depuis le curseur figé, curseur caché sous la fenêtre de l'esclave Windows (`remote/windows/hider.rs`, réutilisée), clavier avalé une fois relâchées les touches tenues ici, table scancode → code evdev, verrous par `GetKeyState`, retour au changement de bureau (Ctrl+Alt+Suppr, Win+L) et quand une fenêtre élevée prend le focus ; hook clavier et fenêtre de masquage posés seulement quand le flux a un client (`Outlet::on_change`), minuterie seulement quand le pointeur est parti. Exemple `lbm-hook --example input_sink` : un esclave qui consigne le flux au lieu de l'injecter, pour essayer un maître sans piloter le bureau de l'esclave |
| Hook esclave | `lbm-hook/src/remote` | Injection `SendInput` (Windows) et uinput (Linux), relâchement de tout ce qui est tenu en fin de session, curseur rangé |
| Interface | `lbm-app` | Page Machines, écrans distants sur la carte, bascule de source, vignettes de fond d'écran distantes |
| Commutation d'entrée | `lbm-agent/src/input_switch.rs` | VCP 0x60 par `ddcutil` ou dxva2, pour un moniteur câblé sur deux entrées sans KVM |
| Fonds d'écran | `lbm-agent/src/peers/wallpapers.rs`, `wallpaper_watch.rs` | Vignettes envoyées par l'esclave, suivies par événements (inotify Plasma, registre et diffusion Windows) |
| Fenêtre élevée chez l'esclave (PR #818) | `lbm-hook/src/remote/blocking.rs`, `remote/windows/foreground.rs`, `Control::InjectionBlocked`, page Machines, tray | Le hook esclave Windows compare l'intégrité du premier plan à la sienne (suivi par `EVENT_SYSTEM_FOREGROUND` pendant une session pilotée seulement), le dit au maître derrière la capacité `InjectionBlocked` ; le maître l'affiche (« Windows blocks the mouse and keyboard on … ») et ramène son pointeur au lieu de le laisser figé |
| Confort (PR #813) | `hook::crossing`, `lbm-agent/src/peers`, tray, `lbm-app` | Indicateur de la machine qui a la souris et le clavier (tray, page Machines ; côté esclave et icône du tray : voir la ligne suivante) ; Verr. Maj/Num/Défil de l'esclave alignés sur ceux du maître à l'`Enter` ; pointeur verrouillé sur une machine par `PointerLockShortcut` (`multi-pc.json`, aucun par défaut), toujours libéré par le secours, la fin du flux, l'arrêt du maître ou le changement de client |
| Raccourcis lus par le maître evdev (PR #826) | `hook/crossing/keys.rs` (`Letters`), `hook/linux/keymap.rs`, `hook/linux/rescue.rs`, `Shared::report_rescue_refused` | Les touches lues par evdev sont des positions : la lettre d'un raccourci (`M` de `Alt+Shift+M` ou du secours `Ctrl+Alt+Shift+M`) est cherchée sur la disposition du bureau, lue dans la keymap que `wl_keyboard` donne à tout client (xkbcommon chargé à l'exécution), suivie à chaque nouvelle keymap ; la disposition active est demandée à KDE (`org.kde.KeyboardLayouts`, suivie par `layoutChanged`, attendue sur le bus si le service arrive après le hook ou redémarre, redemandée à chaque keymap si `/Layouts` n'est pas encore publié) parce que le compositeur peut réserver `wl_keyboard.modifiers` au client qui a le focus, et une keymap à plusieurs dispositions sans réponse est lue dans la première (le journal le dit) ; table US sans Wayland ni xkbcommon. Sur un clavier AZERTY, le verrou et le secours attendaient jusque-là la touche marquée `,`. Le secours est aussi reconnu dans les touches des claviers non capturés, tenu le même temps, tant que le bureau ne l'a pas lié lui-même dans une session dont les signaux sont écoutés ; sa disponibilité suit la liste réelle des claviers (branchement tardif, retrait du dernier, routeur arrêté), et un portail qui ne répond plus le rend aux claviers. Le hook se déclare au portail sous l'app id `io.github.mgth.LittleBigMouse` (`org.freedesktop.host.portal.Registry`) avant de demander le raccourci global |
| Machine pilotée, des deux côtés | `lbm-agent/src/peers/actor.rs` (`forward`), `icons.rs`, `tray`, `lbm-ui/src/machines.rs` | L'esclave sait qu'il est piloté : `PeersState::driven_by` / `Snapshot::DrivenBy` (additifs), tirés des `Enter`/`Leave` du flux du maître tels qu'ils atteignent le hook, la fin du flux (lien, session finie ici, `InputReset`) valant `Leave` ; sa page Machines dit « Driven by <maître> » et marque le maître « Drives this machine ». L'icône du tray change à la traversée, sans survol ni scrutation : **estompée** sur le maître dont le pointeur est parti, **pastille magenta** en bas à droite sur l'esclave piloté. La pastille est peinte dans les pixels de l'icône plutôt que confiée à l'`OverlayIconPixmap` du StatusNotifierItem (que tous les hôtes ne dessinent pas, et que la zone de notification Windows n'a pas) ; `NeedsAttention` a été écarté (sens d'alerte, clignotement selon l'hôte) |
| Autres outils de partage | `lbm-ipc/src/sharing.rs`, `lbm-agent/src/input_sharing.rs`, `sharing_exits.rs`, tray, `lbm-app` | Deskflow (`deskflow-core`, `deskflow`, `deskflow-server`, `deskflow-client`), Synergy (`synergy-core`, `synergys`, `synergyc`), Barrier (`barriers`, `barrierc`) et Input Leap (`input-leaps`, `input-leapc`) sont cherchés quand le mode passe à actif, quand un lien avec une machine s'établit, au Start du moteur et à l'abonnement d'une fenêtre (le tray s'abonne au démarrage de l'agent). Le résultat est dans l'état de l'agent (`Snapshot::input_sharing`, `InputSharing`, absent quand il n'y en a pas), et l'outil est nommé dans le tray (« Deskflow is also running and can move the mouse to other machines »), sur la page Machines et dans la barre d'état. Une connexion établie vers une autre machine (pas la boucle locale) est attribuée au processus (Linux : sockets de `/proc/<pid>/fd` croisés avec `/proc/net/tcp{,6}` ; Windows : `GetExtendedTcpTable`, qui donne le pid) : « …, and it is connected to one now ». Une fenêtre Deskflow seule est dite « open » et non « running », y compris dans la barre d'état (pastille grise « Deskflow open », l'ambre « … also running » étant réservée à un processus qui route ou peut router). Chaque processus trouvé est attendu jusqu'à sa fin, sans scrutation (Linux : pidfd ; Windows : `RegisterWaitForSingleObject` sur un handle `SYNCHRONIZE`) : quitter Deskflow éteint aussitôt l'indication |
| Règles sur l'esclave (cette branche) | `lbm-app/src/remote_rulers.rs`, `lbm-agent/src/peers/overlay.rs`, `lbm-overlay/src/standalone.rs` | « Rulers on screens » dessine aussi les règles sur les écrans de l'esclave : calculées par le maître en mm et dans les pixels de l'esclave, envoyées par `Overlay` (capacité `RemoteOverlay`), contrôlées puis dessinées par l'esclave, inertes (le clic traverse), retirées à la chute du lien, à la fermeture de l'UI du maître, à la coupure du mode et au changement d'écrans ; activables aussi quand l'écran sélectionné est celui de l'esclave (elles le mesurent alors, partout) |

Règle de sûreté visée : **quand un maillon du flux tombe, le pointeur est chez
lui**. Le maître ramène son pointeur et relâche ses claviers ; l'esclave relâche toutes
les touches et tous les boutons que le flux a laissés enfoncés. Le raccourci de secours
reste reconnu pendant que le clavier est capturé. Le hook de l'esclave qui perd seul
sa connexion à son agent en pleine session y est soumis aussi, comme une traversée qui
ne trouve pas de hook chez l'esclave : l'agent de l'esclave dit `InputReset` et le
maître ramène son pointeur.

## Ce qui a été essayé pour de bon

Une seule paire complète entre deux vraies machines : **maître Linux (KDE Plasma 6,
Wayland, backend evdev) → esclave Windows 11**, avec un écran partagé par KVM. Le maître
Windows a tourné sur la vm124 face à un esclave Linux de test (ci-dessous). L'esclave Linux et les fonctions
Windows annexes ont été essayés à part le 2026-11-24, avec `examples/peer_drive` dans le
rôle du maître. Le numéro de PR d'une ligne renvoie à la PR qui a apporté les correctifs
trouvés pendant l'essai.

| Essai | Résultat |
|---|---|
| Appairage TLS entre les deux machines, reconnexion automatique | Vérifié |
| Esclave Windows : pointeur au pixel, clic gauche, touches (étendues comprises), relâchement au `Leave` | Vérifié |
| Publication des écrans de l'esclave, profil existant retrouvé à travers le KVM | Vérifié |
| Traversée maître → esclave et retour | Vérifié (souris scriptée et à la main) |
| Clavier depuis le maître, page Machines | Essayé à la main par le mainteneur, sans défaut signalé |
| Curseur masqué sur l'esclave Windows et sur le maître KWin | Vérifié |
| Vignettes de fond d'écran distantes sur la carte | Vérifié à la liaison |
| Esclave Linux (KWin 6, deux écrans d'échelles 1,66 et 1,25) : pointeur au pixel, clics gauche/droit/milieu, molette verticale et horizontale haute résolution, touches étendues, relâchement au `Leave` (bouton puis touches en ordre inverse) et à la chute du lien (1 ms si le maître meurt, 3 s par keepalive s'il se fige) | Vérifié avec `peer_drive`, après les deux correctifs de la PR #807 |
| Fonds en direct, esclave Windows : diaporama, `SystemParametersInfo`, `IDesktopWallpaper`, clic dans Paramètres, Spotlight, passage en diaporama | Vérifié sur la vm124 : nouvelle vignette chez le maître en 1 à 2 s |
| Injection dans une fenêtre élevée (esclave Windows) | Vérifié sur la vm124 : rien ne passe depuis un agent non élevé ; tout passe (mouvement, clics, touches) depuis un agent élevé, par tâche ou par `StartElevated` |
| Avertissement « fenêtre élevée » (PR #818) : hook esclave non élevé, clavier visuel (`osk.exe`, High + UIAccess) mis au premier plan par son icône de la barre des tâches | Vérifié sur la vm124 : blocage signalé à l'instant où `osk.exe` prend le focus (~300 ms après le survol de l'icône), redit au début de la session suivante ; curseur effectivement figé. Même chose, naturellement, avec la console `mmc.exe` élevée du mainteneur au premier plan. Hook élevé : rien de signalé, tout passe. `SendInput` renvoie le compte complet et `GetLastError` = 0 malgré le blocage. Essayé hook seul (à côté de l'agent du mainteneur, que le mutex d'instance empêche de doubler), pas la chaîne réseau jusqu'au maître, couverte par les tests sur boucle locale |
| Règle de pare-feu 24817 posée et retirée par l'installeur | Vérifié sur la vm124 (PR #811) |
| Règles sur l'esclave Windows (vm124, 4K à 1,5 + 1080p) : affichées à leur place sur les deux écrans, zone mesurée qui suit l'écran choisi, mise à jour, retirées sur ordre et à la chute brutale du lien (maître tué), pointeur qui les traverse (`WindowFromPoint` rend la fenêtre du dessous) | Vérifié avec `peer_drive` comme maître et `peer_remote` comme esclave (l'agent installé du mainteneur tournait à côté, intact) |
| Règles sur un esclave Linux : surfaces layer-shell, mise à jour, bail renouvelé, hôte d'overlay disparu à la fin | Vérifié dans un KWin imbriqué sans écran (`kwin_wayland --virtual`), pas sur un vrai bureau |
| Hook souris bas niveau de Windows, au bord du bureau (sonde, vm124) | Le point rapporté **sort du bureau** : curseur à x = 0 poussé à gauche → -5, bord droit 5759 → 5763, bord haut → -4. Avalé, le mouvement laisse le curseur en place et le point vaut curseur + déplacement accéléré ((7, 3) injecté → (9..11, 4..5)). D'où le choix : avaler et lire le déplacement, pas Raw Input |
| Maître Windows (vm124, 4K à 1,5 + 1080p) → esclave Linux de test (agent `--fake-hook` + `input_sink`, port 24827, aucun pilotage du bureau Linux) : traversée par le bord droit et retour, mouvement, boutons gauche et droit, molette dans les deux sens, touches (lettre, Maj+lettre, Ctrl droite, flèche étendue, Verr. Maj), Verr. Maj tapé là-bas sans effet ici, modificateur tenu en traversant qui garde le clavier ici jusqu'au relâchement, curseur caché (`GetCursorInfo` : `hCursor` nul) puis rendu | Vérifié le 2026-11-24, souris et clavier simulés par `SendInput` relatif et par scancode depuis la session (pas une vraie souris) |
| Maître Windows : secours tenu là-bas (`M` final retenu, `Leave` qui relâche Ctrl/Alt/Maj, pointeur rendu là où il était parti), hook de l'esclave tué en pleine session (retour en ~1 s sans mouvement), lien tué (agent esclave tué net), traversée sans hook chez l'esclave (`InputReset` immédiat), pointeur verrouillé là-bas puis libéré par `Ctrl+Alt+L` | Vérifié le 2026-11-24 sur la vm124 |
| Maître Windows et fenêtre élevée au premier plan | Vérifié : hook non élevé → ni mouvement ni touche vus, pas de traversée, la frappe va à la fenêtre élevée ; une fenêtre élevée **survolée sans le focus** ne gêne pas ; une fenêtre élevée qui prend le focus pendant que le pointeur est parti le ramène au tick suivant (`powershell.exe has the focus above this hook…`) ; agent (donc hook) élevé → tout passe, traversée et frappe comprises |

Jamais essayé sur du vrai matériel, bien que codé et couvert par des tests :

- le maître Windows avec une **vraie souris et un vrai clavier** (seulement `SendInput`
  depuis la session), face à un **esclave Windows** ou à un esclave Linux qui injecte
  pour de bon, ni le passage par Ctrl+Alt+Suppr ou Win+L pendant que le pointeur est
  parti (codé : retour au changement de bureau) ; l'alignement des verrous à l'`Enter`
  n'a pas été vu chez l'esclave (le faux hook de l'esclave de test n'annonce pas
  `InputLocks`, son agent retire donc l'événement), seule la lecture `GetKeyState` côté
  maître l'a été ;

- la commutation VCP 0x60 (ni `ddcutil`, ni dxva2) ;
- le suivi en direct des fonds d'écran sous Linux au-delà des tests (dialogue KDE) ;
  côté Windows, le changement de diaporama sur minuterie et celui quotidien de Spotlight ;
- un esclave Linux piloté par un vrai maître (seulement par `peer_drive`), et son hook
  tué en pleine session ;
- le masquage du curseur maître sous X11 (XFixes) et sous un compositeur autre que KWin ;
- la molette et les boutons autres que le gauche **sur l'esclave Windows** (vérifiés sur
  l'esclave Linux), la correspondance verticale exacte à la traversée ;
- les quatre scénarios de panne corrigés pendant la revue (débordement de file,
  bascule d'esclave, hook redémarré, client du flux remplacé) : prouvés par tests, pas
  provoqués sur les machines ;
- les règles sur l'esclave **depuis `lbm-app`** sur un vrai maître (seul le chemin
  `peer_drive` → esclave a tourné ; le calcul côté `lbm-app` et le retrait à sa fermeture
  sont couverts par des tests), et sur un vrai bureau Linux esclave ;
- l'indicateur côté esclave et l'icône du tray marquée (estompée / pastille) : ni la
  pastille dans le tray Plasma ni l'icône redessinée dans la zone de notification
  Windows n'ont été vues à l'écran ; le calcul des pixels et le changement d'icône sont
  couverts par des tests ;
- les trois fonctions de la PR #813 : l'indicateur, l'alignement des verrous (`GetKeyState`
  depuis le fil de l'injecteur Windows peut retarder sur l'état système ; un esclave
  Linux dont le hook a capturé les claviers lit des LED que le compositeur ne peut plus
  écrire ; délai entre la frappe injectée et la LED) et le pointeur verrouillé sous un
  vrai compositeur (verrouillé chez soi, le raccourci atteint aussi le bureau).
  Essayé le 2026-11-24 (maître KDE Wayland evdev, clavier AZERTY) : le verrou ne
  marchait pas et le portail refusait le secours (« An app id is required »). Corrigé
  par la PR #826, pas encore réessayé : ni la keymap lue sous KWin, ni le secours tenu
  sur les claviers lus, ni la réponse du portail à l'app id déclaré (l'id n'a pas de
  `.desktop` installé à son nom ; un portail qui en exigerait un refuserait encore, et
  le secours lu au clavier resterait seul sur le backend evdev).

## Faiblesses connues

- Fonds en mode « Span » sous Linux à une échelle différente de 1 : mal positionnés, y
  compris dans l'aperçu local.
- Retour d'erreur vers l'interface (#808) : un esclave dont le lien tombe avant de
  répondre à une commutation d'entrée, ou un esclave plus ancien, ne laisse aucun échec
  chez le maître ; l'échec d'une connexion à une adresse tapée n'apparaît que comme
  l'erreur de cette requête. L'interface elle-même n'a été vue que par les tests
  kittest, jamais à l'écran.
- Règles sur l'esclave : elles ne se tirent pas (inertes là-bas, on aligne en tirant
  celles des écrans du maître) ; sous Linux, l'esclave renouvelle le
  bail de l'hôte d'overlay toutes les 500 ms tant que des règles sont affichées (pas
  au-delà).
- Deux maîtres liés au même esclave partagent sa connexion unique au hook (une session
  par connexion). Quand le flux de l'un se termine, la connexion est fermée pour que le
  hook relâche ce qu'il tenait : la session de l'autre, s'il pilotait, se termine aussi.
  Il en est averti (`InputReset`, son pointeur rentre) et `DrivenBy` s'efface (revue de
  la #816).
- Le premier lien avec un esclave qui apporte des écrans inconnus crée un profil
  **désactivé** : le moteur s'arrête jusqu'à ce que l'utilisateur l'active.
- Esclave Linux : si son propre backend evdev route, une souris locale reprend d'où ce
  backend avait laissé le curseur, pas d'où le maître l'a laissé. Deux autres défauts
  trouvés à l'essai sont corrigés (PR #807) : un esclave
  dont le moteur local est arrêté n'injectait pas le pointeur (il ne connaissait son
  bureau que par un `Load`), et KWin 6 jette ce qu'un périphérique uinput émet dans ses
  ~300 premières millisecondes, d'où un premier `Enter` sans effet.

- Autres outils de partage : la liste n'est relevée qu'à certains moments (mode activé,
  lien établi, Start, fenêtre ouverte) ; un processus trouvé en sort dès qu'il se
  termine (événement de fin), mais un Deskflow **lancé** après le dernier de ces moments,
  ou dont la fenêtre démarre ensuite son `deskflow-core`, n'apparaît qu'au moment
  suivant : aucun des deux systèmes n'offre à un agent non privilégié d'événement
  « processus lancé » sans scrutation (Linux : le connecteur de processus netlink exige
  `CAP_NET_ADMIN` ; Windows : `Win32_ProcessStartTrace` exige l'administrateur, et
  `__InstanceCreationEvent` scrute, clause `WITHIN`). De même, une fenêtre Deskflow qui arrête
  son serveur sans se fermer laisse « Deskflow open » à juste titre, mais un
  `deskflow-core` resté en vie après un « Stop » reste dit « running ». Un processus
  qu'on ne peut pas attendre (pidfd refusé, handle d'un processus élevé refusé sous
  Windows) reste dit jusqu'au relevé suivant. Sous Linux, les sockets d'un processus d'un autre utilisateur sont
  illisibles : sa connexion reste « inconnue », même si une connexion est établie sur
  le port 24800 (elle peut être celle d'un autre outil). Les connexions vers la boucle
  locale (socket de contrôle du service, 24801 chez Synergy) ne comptent jamais. Sous Windows, la ligne de commande d'un autre processus
  n'est pas lue : `deskflow-core.exe` n'est dit serveur ou client que par une connexion
  sur le port 24800. Rien de tout cela n'a tourné sur Windows (ni l'attente de fin) ;
  sous Linux, la fenêtre `deskflow` seule a été vue sur une vraie machine, mal nommée
  « running » dans la barre jusqu'à la correction (v6 d51f72d7, signalé par le
  mainteneur), et jamais un `deskflow-core` relié à un client. L'attente de fin a tourné
  sur de vrais processus Linux de test (scripts shell nommés comme Deskflow), pas sur
  Deskflow lui-même. Un outil quitté entre le relevé et la mise en attente est dit
  terminé aussitôt (Linux : `ESRCH` ; Windows : `OpenProcess` en
  `ERROR_INVALID_PARAMETER`, distingué d'un refus d'accès).

- Maître Windows : un pointeur qui donne des positions absolues (tablette, bureau à
  distance, souris d'une VM vue par RustDesk) n'a pas de déplacement à donner ; le
  pointeur de l'esclave saute avec lui. Pendant que le pointeur est parti, les touches
  sont lues par scancode : la lettre du secours et celle du verrou sont cherchées sur la
  disposition de la fenêtre au premier plan (`MapVirtualKeyEx`, relue quand elle change,
  `Letters` de la PR #826) ; ici, le secours reste à `RegisterHotKey`, qui suit la
  disposition, et les touches lues par le hook clavier ne le déclenchent pas une seconde
  fois (`rescue_bound_by_desktop`). Couvert par un test AZERTY, jamais essayé avec un
  vrai clavier AZERTY (la frappe de l'essai visait les touches par leur place US). Une
  fenêtre élevée qui prend le
  focus ramène le pointeur sans que l'interface le dise (seule une ligne du journal du
  hook). Le maître et l'esclave d'une même machine partageraient la fenêtre qui cache
  le curseur (un seul hider par processus).
- Pointeur verrouillé (règle commune, vu sur le maître Windows, vaut aussi pour evdev) :
  chaque report retenu par le verrou remet le moteur à zéro (`begin_touch`) avant de
  repartir de la position tenue ; le report suivant est alors placé par la seule
  contiguïté des pixels et peut passer sur un autre écran du même esclave sans suivre
  les liens du layout. Non corrigé.

## Matrice de support

| | Maître | Esclave |
|---|---|---|
| **Linux, backend evdev** | Livré, vérifié sous KDE Wayland | Livré, vérifié sous KWin 6 avec `peer_drive` comme maître (correctifs de la PR #807) |
| **Linux, backend portail Wayland ou X11** | Absent : les zones distantes ne sont jamais franchissables | Livré si `/dev/uinput` est accessible en écriture, jamais essayé sur matériel |
| **Windows** | Livré (PR #828), essayé sur la vm124 face à un esclave Linux de test, avec une souris simulée ; jamais face à un esclave Windows ni avec une vraie souris | Livré, vérifié |
| **macOS** | Absent | Absent |

Prérequis des rôles Linux, qui ne sont pas les mêmes :

- **maître** : le backend evdev, donc `/dev/uinput` accessible en écriture **et** au
  moins une souris physique lisible (`evdev/devices.rs`, `available`) ;
- **esclave** : seulement `/dev/uinput` accessible en écriture. L'injecteur est installé
  par le hook quel que soit le backend de capture locale ; une machine qui route sa
  propre souris par le portail ou X11 peut donc être esclave.

Et par fonction annexe :

| Fonction | Linux | Windows |
|---|---|---|
| Publication des écrans | KScreen, sinon XRandR | Oui |
| Lecture du fond d'écran | Plasma seulement | Oui |
| Suivi du fond en direct | Plasma seulement | Oui, vérifié sur la vm124 |
| Curseur masqué, côté maître | Wayland avec layer-shell (KWin vérifié), X11 par XFixes ; rien sous GNOME, qui n'a pas layer-shell | Oui (fenêtre d'un pixel sous le curseur figé, au centre d'un écran), vérifié sur la vm124 |
| Curseur rangé, côté esclave | Rien de visible. Un éclair du curseur au centre à la fin de session (signalé par le mainteneur) venait de l'ordre : le pointeur sautait avant que la fenêtre n'ait traité `WM_SETCURSOR` ; l'image est maintenant effacée avant le saut (`PARK_ORDER`), **raisonné, pas encore revu à l'écran** | Oui |
| DDC/CI | `ddcutil` | dxva2 |
| Profil conservé à travers un KVM | Oui si la dalle a une clé commune aux deux machines **et unique parmi les écrans** (l'id du moniteur est alors cette clé). Sans numéro de série dans l'EDID, ou si le KVM le masque, l'écran distant prend un id propre à sa source et la bascule sélectionne un autre profil. Avec deux dalles annonçant le même numéro de série, celle que le KVM bascule est rattachée à sa jumelle restée sur le maître, et le profil change aussi | Oui, sur un maître Windows, depuis la PR #827 : l'écran distant prend l'id que Windows lui donne en local (`{pnp}{série}_{semaine}_{année}_{checksum}`), calculé sur l'EDID envoyé par l'esclave, celui que le KVM fait passer tel quel. Marche aussi sans numéro de série. Avec deux dalles d'EDID identiques, l'écran resté sur le maître garde l'id, la jumelle distante prend celui de sa source. Jamais essayé sur du vrai matériel (le maître Windows non plus), couvert par des tests |

## Reste à faire pour un support Linux/Windows universel

Dans l'ordre que je propose. Les estimations d'ampleur sont les miennes, pas des
mesures.

### 1. Valider ce qui est déjà codé

Avant d'écrire du code neuf, lever les « jamais essayé » ci-dessus. Trois d'entre eux
ne demandent que du temps sur les machines actuelles :

- ~~**Esclave Linux**~~ : fait avec `peer_drive` (PR #807) ; reste un essai entre deux
  vraies machines Linux.
- **VCP 0x60** sur un moniteur câblé sur deux entrées, dans les deux sens. Le Samsung
  G80SD n'a pas de VCP : il faut un autre écran.
- **Fonds en direct** : changements immédiats vérifiés sous Windows (vm124) ; restent
  le dialogue KDE, et sous Windows l'avance du diaporama sur minuterie et le
  renouvellement quotidien de Spotlight.

### 2. Maître Windows

Fait (PR #828) ; voir « The Windows master » dans [`multi-pc.md`](multi-pc.md).
Ce qui a été tranché, et pourquoi :

- **Mouvement relatif pendant que le pointeur est parti** : avaler chaque mouvement et
  lire `point rapporté - curseur`. Essai sur la vm124 : le hook bas niveau rapporte bien
  au-delà du bord du bureau (x = -5 au bord gauche), donc la traversée vers une zone
  distante se fait comme entre deux écrans ; un mouvement avalé laisse le curseur en
  place, et le point rapporté vaut curseur + déplacement **accéléré**. Raw Input donne
  les comptes de la souris avant accélération (il faudrait refaire la courbe de Windows
  pour garder la sensation) et arrive en messages de fenêtre, à part des boutons et des
  touches du hook. Le curseur figé est ensuite rangé au centre d'un écran, sous la
  fenêtre de masquage, pour qu'aucun bord ne borne le report suivant.
- **Capture du clavier** : hook clavier bas niveau posé seulement quand le flux a un
  client ; il avale les touches une fois relâchées celles qui étaient tenues ici à
  l'`Enter` (règle de Linux : un clavier n'est saisi que quand rien n'y est tenu) ;
  table `remote::keymap::linux_key`, l'inverse exacte de celle de l'esclave (testée sur
  tous ses codes). Ctrl+Alt+Suppr et Win+L ne passent par aucun hook : le changement de
  bureau ramène le pointeur.
- **Curseur masqué** : `remote/windows/hider.rs` réutilisé tel quel, avec la création
  de la fenêtre avancée à l'arrivée du client (jamais d'attente dans le hook).
- **Automate** : `hook::ll_master` implémente `Platform` ; une « trame » est un
  événement, passé ou avalé. Secours, pointeur verrouillé et verrous fonctionnent comme
  sous Linux (essayés) ; la lettre des raccourcis suit la disposition du premier plan.
- **Identité des moniteurs** : PR à part (#827), l'id Windows calculé sur l'EDID de
  l'esclave.
- **Fenêtres élevées** côté maître : vérifié, un hook non élevé ne voit rien de
  l'entrée destinée à une fenêtre élevée au premier plan (ni mouvement ni touche), même
  le pointeur ailleurs ; survolée sans le focus, elle ne gêne pas. Le maître ramène son
  pointeur quand une telle fenêtre prend le focus pendant qu'il est parti. Le recours
  est `StartElevated` (essayé : agent élevé, hook élevé, tout passe) ; pour un
  utilisateur standard, le service Windows.

Reste : un essai avec une vraie souris et un vrai clavier, face à un esclave Windows et
à un esclave Linux qui injecte, et avec un clavier AZERTY pour les raccourcis ; dire
dans l'interface qu'une fenêtre élevée a ramené le pointeur.

### 3. Esclave Windows : les deux trous

- **Fenêtres élevées** : mouvements, clics et touches passent tous par le même
  `SendInput`, soumis à UIPI : depuis un hook non élevé, rien n'est garanti quand une
  fenêtre élevée a le focus. Une option existe déjà : `StartElevated` lance, pour un
  administrateur à jeton fractionné, un agent élevé dont le hook hérite du niveau
  d'intégrité (`lbm-agent/src/elevation.rs`). **Essayé sur la vm124** : depuis un agent
  non élevé, rien ne passe, pas même le mouvement ; depuis un agent élevé, tout passe.
  Depuis la PR #818, LBM le dit : le hook compare le niveau d'intégrité (et l'UIAccess)
  du processus au premier plan au sien, le maître affiche « Windows blocks the mouse and
  keyboard on <esclave>: an elevated window has the focus » avec la sortie (« Start with
  elevated privileges ») et ramène son pointeur au lieu de le laisser figé. Le premier
  plan est suivi par `EVENT_SYSTEM_FOREGROUND` pendant la session, et chaque lot injecté
  le compare à celui déjà jugé : à l'essai, l'événement a manqué une fois sur six
  (le crochet de focus du hook aussi), d'où ce filet. Limites : seul le **premier plan**
  compte — un clic sur une fenêtre élevée qui n'a pas le focus peut être jeté sans rien
  dire, le mouvement passant toujours (non essayé) ; entre deux sessions le premier
  plan n'est pas suivi, l'avertissement du maître reste donc celui de la dernière
  session jusqu'à la traversée suivante ; un jeton illisible est pris pour un niveau
  supérieur. Un événement est traité en relisant le premier plan sous le même verrou
  que les lots (il arrive en retard, d'une file). Un `SendInput` incomplet sans
  différence d'intégrité vue (bureau sécurisé) est dit « cause inconnue », sans conseil
  d'élévation. Le retour du pointeur chez le maître n'a été vu qu'en test (et par
  `peer_drive`, qui l'imite), pas avec un vrai maître.
  Aucun code de plus n'est nécessaire pour un administrateur, hormis un correctif :
  l'agent lisait `StartElevated` dans la configuration réelle et non dans celle de
  `--config-dir`, et les chemins relatifs n'étaient pas rendus absolus avant la relance élevée (PR #811). Le refus de l'invite UAC n'a pas été observé. Un utilisateur standard reste sans solution hors
  service Windows.
- **Bureau sécurisé** (UAC, écran de verrouillage, Ctrl+Alt+Suppr) : hors d'atteinte
  d'un processus de session. Il faut un service. C'est ce qui a bloqué un essai pendant
  le développement : la machine était verrouillée.

**Le service Windows est livré dans `v6`** (conception :
[`multi-pc-service-windows.md`](multi-pc-service-windows.md), option B validée le
2026-11-24 ; #823 à #832). Il couvre les deux trous, y compris pour un utilisateur
standard. Son point principal : un canal agent → service limité à l'utilisateur ne
suffit pas (tout processus de l'utilisateur peut se faire passer pour l'agent, donc
valider une invite UAC) ; le service reçoit lui-même le flux, en TLS mutuel, des seuls
maîtres qu'un administrateur a approuvés.

- **Pièces** : `lbm-inject` (#824), l'injecteur lancé en SYSTEM dans la session console,
  qui suit le bureau d'entrée ; `lbm-service` (#825), le service, son magasin de maîtres
  approuvés (`lbm-service trust | untrust | list`, en administrateur) et `lbm-admin-only` ;
  son écoute sur 24818 et l'offre publiée sous `HKLM\SOFTWARE\LittleBigMouse\InputService`
  (#830) ; le maître qui y envoie le flux (#829) ; l'agent qui annonce l'offre (#831) ;
  une tâche décochée de l'installeur, la CI et SignPath (#832).
- **Essayé pour de bon sur la vm124** (compte administrateur), voir « Essai… » dans la
  conception : depuis la session 0, invite UAC annulée par Échap injecté et fenêtre
  élevée au premier plan pilotée ; sous le SCM, avec un maître de test Linux
  (`examples/master_trial`), pointeur au pixel près, invite UAC annulée, maître non
  approuvé refusé ; révocation par vidage du magasin et rechargement (contrôle SCM 128,
  pas la commande `lbm-service untrust` de bout en bout), qui coupe aussitôt et relâche
  Ctrl ; l'installeur réel : installation, mise à jour avec et sans la tâche,
  désinstallation — la mise à jour essayée supprimait puis recréait le service, la
  version livrée le reconfigure en place (`sc config`, #832), dont seule la commande a été
  essayée, sur un service factice.
- **Jamais tourné sur du vrai matériel** : la chaîne complète avec un vrai agent maître,
  un vrai agent esclave qui annonce l'offre et le service (le routage du maître, #829,
  n'est testé que sur sockets locaux ; la lecture et la notification de l'offre, #831,
  sous une clé `HKCU` isolée, aussi sous Wine) ; `lbm-service trust` / `untrust` de bout
  en bout (dialogue de confirmation, notification du service en marche) ; la mise à jour
  par l'installeur livré (`sc config`) ; l'écran de verrouillage *via le service* (atteint sans lui, effet
  jamais vu à l'œil) ; un compte standard (aucun sur la vm124) ; Ctrl+Alt+Suppr (non
  codé, option `SendSAS` proposée) ; l'écran de connexion avant toute session (hors de
  cette série) ; changement rapide d'utilisateur, veille ; deux maîtres approuvés à la
  fois (testé sur boucle locale) ; l'installeur interactif et son code de sortie 100.
- **À faire par le mainteneur** : ajouter `lbm-service.exe` et `lbm-inject.exe` à la
  configuration d'artefact SignPath si elle liste les exécutables par nom.
- **Reste** : épingler côté maître l'empreinte du service (aujourd'hui vérifiée contre
  l'annonce de l'agent à chaque liaison) ; l'état « piloté par » côté esclave quand le
  service porte le flux ; une interface pour `trust` (aujourd'hui en ligne de commande).

### 4. Maîtres Linux sans evdev

Le backend evdev demande l'accès à `/dev/uinput`. Sans lui, LittleBigMouse retombe sur
le portail Wayland ou sur X11, et le mode multi-PC n'y existe pas.

- **X11** : faisable (XInput2 donne le mouvement brut, `XGrabKeyboard` la capture).
- **Portail Wayland** : le backend existant (`hook/linux/portal.rs`) ouvre déjà une
  session `org.freedesktop.portal.InputCapture`, se connecte à EIS et reçoit les
  déplacements relatifs pendant la capture. Il ne demande aujourd'hui que le pointeur.
  Reste à demander et traiter le clavier, à rendre les zones distantes franchissables
  pour ce backend et à relayer les événements pendant le séjour sur l'écran distant —
  d'où l'intérêt de l'automate de traversée partagé (point 2). Le support dépend du
  compositeur.

À mon avis, documenter « le maître Linux demande evdev » suffit dans un premier temps.

### 5. Bureaux Linux autres que KDE

- Fond d'écran : lecteur et suivi pour GNOME (`gsettings`/dconf) au minimum.
- Curseur du maître sous GNOME : pas de layer-shell, donc pas de masquage aujourd'hui.
- Esclave Linux : aucun moyen portable de cacher le curseur du compositeur.
- Détection d'écrans hors KDE : XRandR seulement. Sous un compositeur Wayland autre
  que KWin, elle passe donc par XWayland ; ce que cela donne n'a pas été regardé.

### 6. Transversal

- **Plusieurs esclaves à la fois.** Le flux hook → agent ne porte pas de destination ;
  un seul esclave est « sélectionné ». Le lever change le protocole du flux d'entrée.
- **Presse-papiers** : texte et image (PR #821) essayés sur la paire réelle, ils passent
  dans les deux sens ; les fichiers (PR #822) passent aussi ;
  glisser-déposer étudié et écarté pour l'instant. Voir la section suivante.
- ~~**Pare-feu Windows**~~ : fait (PR #811). L'installeur
  Inno Setup (`LittleBigMouse.Setup/LittleBigMouse.iss`) pose la règle TCP 24817 pour
  `lbm-agent.exe`, sous-réseau local, tous profils (le profil « Public » est celui
  d'un nouveau réseau sous Windows 11 : une règle « Privé » y resterait inactive ; à
  rediscuter si besoin), et la retire à la désinstallation. L'essai a révélé que
  l'installeur v6 **n'embarquait pas `lbm-agent.exe`** et que la désinstallation
  tuait le hook avant l'agent, qui le relançait ; les deux sont corrigés. La CI copie
  désormais l'agent à côté de l'interface et la release signée exige sa signature :
  **la configuration SignPath doit inclure `lbm-agent.exe`** (à faire à la main). Non
  essayée : la mise à jour par-dessus une installation précédente.
- ~~**Retour d'erreur vers l'interface**~~ : PR #808.
- **Profil désactivé au premier lien** : décider si un esclave nouvellement appairé
  doit activer son profil d'office.
- **Traductions** de la page Machines : non vérifié.
- **Tests de bout en bout** avec les vrais binaires sur deux machines : aujourd'hui les
  tests d'intégration tournent en boucle locale avec un faux hook, et la revue a montré
  deux fois que cette doublure masquait un défaut.
- ~~Les deux faiblesses de protocole~~ : corrélation `Loaded` réglée (PR #809 : identifiant facultatif dans `Load`, renvoyé par `Loaded`/`LoadFailed`) ; `InputReset` quand le hook de l'esclave tombe (PR #806).

## Presse-papiers partagé

Palier 1 (**texte et image**, dans les deux sens) codé : PR #821. Palier 3 (**fichiers**)
codé : PR #822. Les trois passent sur la paire réelle. Glisser-déposer étudié, non codé.
Le contrat est dans [`multi-pc.md`](multi-pc.md), « Shared clipboard ». Ce qui suit dit
ce qui a été tranché, et ce qui n'a jamais tourné.

### Ce que KWin offre vraiment

Vérifié le 2026-11-24 sur la machine du mainteneur (KWin et Plasma 6.8.3, Wayland) avec
`wayland-info`, en client ordinaire : **`ext_data_control_manager_v1` version 1** est
annoncé, `zwlr_data_control_manager_v1` ne l'est pas. Le code ne parle donc que
`ext-data-control-v1` (crate `wayland-protocols` 0.32, fonction `staging`) ; un
compositeur qui n'aurait que `wlr-data-control` (Sway ancien, Hyprland ancien) n'est pas
pris en charge, l'état le dit. GNOME n'a ni l'un ni l'autre.

### Les deux questions, tranchées

- **Annonce immédiate**, pas de synchronisation au passage du bord. Chaque copie est
  annoncée tout de suite (quelques octets), le contenu ne traverse qu'à la lecture. Se
  synchroniser au passage du bord obligerait le presse-papiers à écouter le flux d'entrée
  — exactement ce que la règle du projet interdit — pour un gain nul, puisque le transfert
  à la demande laisse déjà une copie jamais collée hors du réseau. Limite connue : un
  lecteur qui lit tout dès l'annonce (Klipper sous KDE lit le texte de chaque sélection)
  fait traverser le **texte** à chaque copie ; sous Windows, l'historique du
  presse-papiers ferait de même, d'où l'exclusion de l'historique et du nuage pour une
  offre venue d'ailleurs.
- **Dans l'agent.** Sous Linux il ouvre sa propre connexion Wayland, sur un fil à lui,
  seulement quand l'option est cochée et qu'une machine qui partage est liée. Le hook n'en
  sait rien.

### Essai réel, et ce qui n'a pas encore tourné

- **Essayé le 2026-11-24 sur la paire du mainteneur** (maître KDE Plasma 6.8.3 Wayland,
  esclave Windows 11 vm124, les deux agents à `f1c04196`, presse-papiers ouvert dans les
  deux sens) : **le texte et l'image passent dans les deux sens.** Côté Linux l'agent a
  ouvert `ext-data-control-v1` auprès de KWin sans erreur.
- Avant cela, les essais étaient deux agents en boucle locale avec un presse-papiers en
  mémoire (`lbm-agent/tests/clipboard.rs`), les conversions (fins de ligne, DIB ↔ PNG),
  le classement des types MIME, et la priorité du flux d'entrée sur un transfert de
  8 Mio au niveau du lien (`lbm-net`).
- Pas encore observé : quelles applications ont servi de source et de cible, une image
  volumineuse sur un réseau lent (Qt abandonne après une seconde sans données), l'effet
  du tampon d'envoi réduit en Wi-Fi, et les points suivants :
  le rendu différé quand une application lit `CF_DIBV5` ou `CF_BITMAP` (que Windows
  synthétise depuis `CF_DIB`), le fil du presse-papiers bloqué pendant un rendu, et
  Klipper qui pourrait reprendre la sélection à son compte quand la source disparaît.
- Effet de bord non mesuré : la mémoire tampon d'envoi réduite à 64 Kio sur tous les
  liens (voir `multi-pc.md`). Sans effet attendu sur l'entrée ; à vérifier en Wi-Fi.

### Formats, par palier

| Palier | Contenu | Windows | Linux | État |
|---|---|---|---|---|
| 1 | Texte | `CF_UNICODETEXT` | `text/plain;charset=utf-8`, `UTF8_STRING`, `text/plain` | Codé (PR #821), passe dans les deux sens (essai réel du 2026-11-24) |
| 1 | Image | `PNG` enregistré, `CF_DIB` | `image/png` | Codé (PR #821), passe dans les deux sens (essai réel du 2026-11-24) |
| 2 | Texte mis en forme | `HTML Format`, `Rich Text Format` | `text/html`, `text/rtf` | Pas commencé ; l'en-tête de `HTML Format` (décalages) est à reconstruire |
| 3 | Fichiers | `CF_HDROP` | `text/uri-list` | Codé (PR #822), fonctionne sur la paire réelle (essai du 2026-11-24) |
| 4 | Glisser-déposer entre machines | — | — | Étudié, non codé : voir plus bas |

Le texte brut d'une source qui n'offre que du HTML n'est pas proposé : la plupart des
applications offrent déjà `text/plain` à côté.

### Fichiers (palier 3)

Le contrat est dans `multi-pc.md`. Ce qui a décidé de la forme :

- **Les deux presse-papiers de fichiers ne portent que des chemins.** Dolphin lit
  `text/uri-list` puis copie depuis ces chemins ; l'explorateur lit `CF_HDROP` puis fait
  de même. Les fichiers doivent donc exister localement au moment où l'application lit :
  ils sont rapatriés d'abord dans un dossier propre au collage, sous le dossier de
  réception, et c'est de là que l'application copie. Conséquence assumée : les fichiers
  existent deux fois sur la machine qui colle (le dossier de réception n'est pas vidé,
  un collage pouvant encore y lire).
- **Combien de temps un lecteur attend.** Vérifié dans les sources : une application Qt
  sous Wayland abandonne après **une seconde sans données**, délai remis à zéro à chaque
  paquet, et bloque son fil d'interface pendant ce temps
  (`QWaylandMimeData::readData`, qtwayland 6.8) ; GTK 4 n'a pas de délai. Windows
  abandonne `WM_RENDERFORMAT` après **30 s**, non réglable (Raymond Chen, *The Old New
  Thing*, 2022-06-09). Garder le tuyau ouvert en y écrivant des commentaires `#` de
  `text/uri-list` pendant le transfert aurait contourné la seconde de Qt, mais **Klipper
  lit les URI de chaque sélection**, dans plasmashell : il l'aurait figé pendant tout le
  transfert. D'où le choix : le rapatriement démarre à la première lecture et continue
  seul ; une lecture qui abandonne avant ne coûte rien ; le collage suivant obtient les
  fichiers aussitôt. L'interface dit « Réception des fichiers (taille) ; collez à
  nouveau une fois arrivés ». Avec Klipper, le rapatriement démarre donc dès la copie,
  dans la borne de 4 Gio.
- **`CFSTR_FILEDESCRIPTOR` / `CFSTR_FILECONTENTS`** (collage différé sous Windows) : ce
  serait la forme juste côté Windows — l'explorateur lit alors le contenu en flux, avec
  sa propre fenêtre de progression et son bouton Annuler, sans copie intermédiaire. Mais
  ces formats ne passent que par un `IDataObject` posé avec `OleSetClipboard` (un objet
  COM, un `IStream` par fichier), ce qui remplace tout le rendu différé de la fenêtre
  message-only, et rien de cela ne peut être vérifié sans exécuter sous Windows. Laissé
  pour une PR qui l'essaiera sur la vm124. Sous Linux il n'y a pas d'équivalent sans
  système de fichiers virtuel (FUSE, kio-fuse).
- **Annulation** : changer de copie, couper la réception ou perdre le lien arrête le
  rapatriement, prévient l'autre machine (`ClipboardAbort`) et supprime ce qui avait été
  écrit. L'application qui attendait reçoit un échec. Il n'y a pas de bouton Annuler.

**Essayé le 2026-11-24 sur la paire du mainteneur** (KDE Plasma 6.8.3 Wayland ↔
Windows 11 vm124, `f1c04196`) : **les fichiers passent.** Non relevé : le sens ou les
deux, les applications, la taille, ni si le premier collage a échoué avant l'arrivée des
fichiers. Avant cela, les essais étaient deux agents en boucle locale (dossier, sous-dossier, fichier de plus d'une plage,
fichier vide, second collage servi sans nouveau transfert, borne, nom hors du dossier
refusé, émetteur parti en plein transfert) et les conversions `CF_HDROP` ↔ chemins et
`text/uri-list` ↔ chemins.

### Glisser-déposer entre machines : étude (palier 4)

Conclusion : **pas faisable proprement aujourd'hui ; non codé.** Le copier-coller de
fichiers couvre le besoin sans les contorsions ci-dessous.

**Ce que font les autres.** Ni Deskflow ni Input Leap ne copient de fichiers par le
presse-papiers (leurs formats : texte, HTML, images). Synergy 1.x avait un glisser-déposer
Windows/macOS, désactivé par défaut, qu'Input Leap garde en l'état et que **Deskflow a
supprimé en mai 2025** (commit `5365e34f`, « broken on all platforms »). Son principe sous
Windows : quand le pointeur quitte l'écran bouton tenu, déplacer une petite fenêtre
cachée, cible OLE (`RegisterDragDrop`), sous le curseur, simuler Échap puis le relâchement
du bouton pour **voler le dépôt** et apprendre le chemin, puis envoyer le fichier (un
seul). Côté cible, seul macOS recréait un glisser (vue Cocoa factice) ; sous Windows le
fichier était simplement écrit sur le Bureau. Linux n'a jamais été pris en charge.

**Détecter qu'un glisser franchit le bord.**
- Windows (source) : le hook voit le bouton tenu, pas ce qui est glissé. Il faudrait une
  cible OLE sous le curseur au moment du franchissement (une bande au bord, montrée
  seulement bouton tenu) : `DragEnter` donne l'`IDataObject`, d'où `CF_HDROP`, sans
  déposer. Reste la boucle `DoDragDrop` de la source, qui garde la capture : pendant que
  le pointeur est sur l'autre machine le hook avale les entrées, la source ne voit jamais
  le relâchement, et il faut l'annuler (Échap simulé) au retour ou au dépôt distant.
- Wayland (source) : un client ne voit un glisser d'un autre client que quand il passe
  sur **sa propre surface** (`wl_data_device.enter`) ; `ext-data-control-v1` ne parle
  pas du tout de glisser. Il faudrait une surface layer-shell en bande au bord, montrée
  seulement bouton tenu, sinon elle volerait les clics du bord (barres de défilement).
  Faisable sous KWin, fragile, et sans équivalent sous GNOME.

**Reprendre le glisser côté cible.**
- Windows : `DoDragDrop` sur le fil de l'agent avec un `IDataObject` à nous ; il suit le
  bouton injecté par `SendInput`, ce qui fonctionne en principe. Mais au dépôt
  l'explorateur demande `CF_HDROP` (fichiers locaux, donc rapatriés avant de répondre,
  interface figée) ou `CFSTR_FILEDESCRIPTOR`/`FILECONTENTS` (le même objet COM que pour
  le collage différé, plus `IDataObjectAsyncCapability`). Synergy n'a jamais fait cette
  moitié.
- Wayland : `wl_data_device.start_drag` exige le *serial* d'un appui de bouton **sur une
  surface du client** (prise implicite). L'agent devrait montrer une surface sous le
  pointeur à l'entrée, y faire arriver l'appui injecté, puis lancer le glisser :
  l'équivalent de la vue factice de Synergy sous macOS, contraire au fonctionnement
  normal du compositeur. Aucun protocole ne permet à un client sans focus de démarrer un
  glisser.

**Ce qui rendrait la chose propre** : côté Windows, l'objet COM
`FILEDESCRIPTOR`/`FILECONTENTS` (utile d'abord au collage, voir plus haut) ; côté Wayland,
un protocole de glisser pour les outils de partage d'entrée (rien n'existe, ni dans
`wayland-protocols` ni dans le portail `InputCapture`). Sans les deux, un glisser
partiel (Windows → Windows seulement) serait possible, au prix de la bande cible et des
Échap simulés de Synergy ; je ne le recommande pas.

### Garde-fous en place

- Option coupée par défaut, par machine et par sens (« Envoyer », « Recevoir » dans la
  page Machines ; `ClipboardSend`, `ClipboardReceive` dans `multi-pc.json`).
- Rien d'ouvert sans option ni machine liée qui partage ; refermé dès que l'une manque.
- Secrets non annoncés (`x-kde-passwordManagerHint`,
  `ExcludeClipboardContentFromMonitorProcessing`).
- Borne de 64 Mio par transfert (`ClipboardMaxBytes`), tenue par les deux côtés. La
  taille n'est connue qu'à la lecture (ni Wayland ni le rendu différé ne la donnent à
  l'annonce) : l'annonce est faite, c'est le collage qui échoue, et l'état le dit
  (`Clipboard.Problem`).
- Pas de réannonce de ce qu'on vient de recevoir (propriétaire sous Windows, type privé
  sous Wayland).
- Sélection primaire et X11 : hors périmètre.

## Fonctionnalités à envisager

Des idées, pas des engagements. Les points 1, 3 et 4 sont livrés (PR #813), jamais
essayés sur matériel ; les autres ne sont pas commencés. Dans l'ordre d'utilité estimée.

1. **Verrouiller le pointeur sur une machine** par un raccourci *(livré, PR #813)*. Pour les jeux et le
   plein écran, où l'on franchit le bord par accident. La reconnaissance d'un raccourci
   dans le flux capturé existe déjà pour le raccourci de secours.
2. **Raccourcis pour sauter d'une machine à l'autre** sans viser un bord, avec retour à
   la dernière position. Utile quand les écrans ne sont pas côte à côte.
3. **Synchroniser les verrous** (Verr. Maj, Verr. Num) à l'entrée sur l'esclave *(livré, PR #813)*.
4. **Indicateur de machine pilotée** *(livré, PR #813, complété ensuite)* dans le tray et
   dans l'état de l'agent. L'essai réel du 2026-11-24 a montré deux trous, comblés
   depuis : l'esclave n'affichait rien (seul le maître savait), et l'infobulle du tray
   ne se lit qu'en survolant l'icône, donc le pointeur sur le maître — jamais quand il
   est ailleurs. D'où l'état « piloté par » de l'esclave et l'icône marquée.
5. **Réveil de l'esclave** : Wake-on-LAN à l'approche du bord, et pas de mise en veille
   de l'écran de l'esclave pendant qu'on le pilote. Le WoL existe déjà dans le dépôt
   pour les téléviseurs.
6. **Verrouiller les deux sessions ensemble** : verrouiller le maître verrouille
   l'esclave. Le déverrouillage à distance dépend du service Windows.
7. **Touches système vers l'esclave** : choisir si Alt+Tab, la touche Windows et
   consorts vont à la machine pilotée ou restent au maître.

Moins prioritaires :

- ~~**Découverte automatique** des machines sur le réseau local~~ *(livrée, voir
  « Découverte automatique » ci-dessous)*.
- **Suivre l'entrée du moniteur** : quand un écran partagé change d'entrée par ses
  propres boutons, basculer la source active. Seulement si le moniteur le signale par
  un événement, le projet excluant la scrutation.
- **Molette et pointeur aux réglages de l'esclave** : vitesse et sens de défilement
  viennent du maître aujourd'hui.

Écartés : le partage audio et le transfert de fichiers autonome, hors du sujet de
LittleBigMouse.

## Découverte automatique

Les agents d'un même réseau local se signalent les uns aux autres ; la page Machines de
`lbm-app` les liste dans « On the network », chacun avec un bouton **Pair** qui fait
exactement ce que fait une adresse tapée (`PeerConnect`). La découverte ne fait que
proposer : l'appairage reste celui de `lbm-net`, TLS mutuel, empreinte confirmée sur les
deux machines.

### Le mécanisme choisi : un datagramme UDP à nous

Trois candidats : mDNS/DNS-SD (`_littlebigmouse._tcp`), une diffusion UDP propre, ou
rien de plus que l'adresse tapée. Critère par critère :

| Critère | mDNS/DNS-SD | Diffusion UDP propre |
|---|---|---|
| Linux à côté d'Avahi | soit passer par Avahi (D-Bus, un démon de plus à exiger), soit un répondeur embarqué qui partage le port 5353 avec Avahi | port à nous, aucun voisin |
| Windows | soit `DnsServiceRegister`/`DnsServiceBrowse` (Windows 10 1809+, un second code), soit le répondeur embarqué qui partage 5353 avec le service DNS Client | le même code que sous Linux |
| Aucune scrutation | un répondeur conforme réannonce et rafraîchit son cache à 80 % du TTL (RFC 6762 § 5.2) : des minuteries tant que le mode tourne | une requête au démarrage, des réponses aux requêtes, un adieu à l'arrêt, rien d'autre |
| Pare-feu Windows | la règle « mDNS (UDP-In) » fournie vise le service DNS Client, pas notre exe : il faut quand même une règle | une règle UDP 24817, posée comme la règle TCP |
| Ce qui est annoncé | nom d'instance, cible, port, enregistrements TXT : à cadrer, et visible de tout navigateur Bonjour | nom, empreinte, port, et rien d'autre (vérifié par un test) |
| Coût | une dépendance lourde ou deux backends | ~250 lignes dans `lbm-net` et `if-addrs` |

mDNS n'apporte rien que la diffusion n'apporte aussi entre deux agents LittleBigMouse —
personne d'autre n'a à les voir — et coûte soit deux backends, soit une cohabitation
délicate sur 5353 plus des minuteries. D'où le datagramme à nous
(`rust/crates/lbm-net/src/discovery.rs`).

### Comment ça marche

- Une socket UDP sur **24817** (le même numéro que le TCP des liens, pour une seule
  chose à retenir dans le pare-feu ; fixe, quel que soit le port TCP choisi, puisqu'il
  faut savoir où demander avant de rien savoir des autres). `SO_REUSEADDR` : un second
  agent sur la même machine (une seconde session Windows) écoute aussi. IPv4 seulement.
- Datagramme JSON d'au plus 1 Kio :
  `{"Lbm": "LittleBigMouse", "V": 1, "Kind": "Query" | "Announce" | "Bye",
  "MachineId": "<sha-256 hex>", "Name": "tower", "Port": 24817}`. Un datagramme qui ne
  se lit pas ainsi (autre en-tête, empreinte mal formée, port nul hors `Bye`) est ignoré.
  Le nom est coupé à 64 caractères, sans caractères de contrôle.
- Au démarrage du mode, une fois le port TCP ouvert : un `Query` envoyé à l'adresse de
  diffusion de **chaque** réseau IPv4 de la machine (la diffusion limitée
  255.255.255.255 ne sort que par l'interface de la route par défaut), répété à 1 s et
  3 s au cas où un datagramme se perde. Chaque `Query` entendu reçoit un `Announce` en
  retour, à l'envoyeur seul. À l'arrêt du mode, un `Bye`. **Aucune minuterie** au-delà
  des deux répétitions ; mode coupé, aucune socket.
- L'état `Peers` gagne `Discovered: [{ MachineId, Name, Address, Paired }]` (absent tant
  que la découverte ne tourne pas) et `DiscoveryError` ; la requête `PeerDiscover`
  (capacité `PeerDiscovery`) vide la liste des machines non appairées et redemande.
  C'est le bouton **Look again** : une machine partie sans `Bye` (coupure, veille) reste
  listée jusque-là, faute de scrutation.
- Une machine **appairée** entendue est reconnue par son empreinte. Si c'est un esclave
  non relié, l'agent l'appelle aussitôt : à l'adresse entendue quand celle enregistrée
  est une adresse IP différente (bail DHCP changé), à celle enregistrée sinon (un nom
  tapé par l'utilisateur survit aux baux). L'adresse enregistrée ne change qu'une fois
  le lien établi **avec la bonne empreinte** (`admit`) : un datagramme menteur fait
  tenter une connexion, qui échoue sur `UnknownFingerprint`, et rien n'est réécrit
  (test `a_datagram_that_lies_about_a_paired_fingerprint_changes_nothing_stored`).
- Une machine sur plusieurs réseaux (un LAN et un pont de conteneurs, filaire et Wi-Fi)
  est entendue sur chacun : l'adresse affichée ne change que pour une source jamais
  entendue d'elle (un bail qui change), pas à chaque datagramme.
- Au plus 64 machines listées : un réseau inondé de fausses empreintes remplit la liste,
  pas la mémoire.

### Pare-feu

- Windows : l'installeur pose une seconde règle, `Little Big Mouse (multi-PC
  discovery)`, UDP 24817 entrant, même portée que la règle TCP (exe de l'agent,
  sous-réseau local, tous profils). Sans elle, un `Query` diffusé par une autre machine
  n'arrive pas ; la machine n'est vue que lorsqu'elle demande elle-même (Windows laisse
  passer les réponses unicast à une diffusion pendant 3 s).
- Linux : rien n'est posé. Un pare-feu local (firewalld, ufw) doit laisser entrer UDP
  24817 depuis le réseau local, comme il doit déjà laisser entrer TCP 24817.

### Essai réel

- **2026-11-25, deux agents jetables sur le poste Linux** (`--fake-hook`, config à part,
  TCP 24900 et 24901, UDP 24817 partagé), par la vraie diffusion du LAN 192.168.4.0/24 :
  chacun voit l'autre dans `Discovered` sans adresse tapée, et le `Quit` de l'un le
  retire de la liste de l'autre (`Bye`). Deux défauts trouvés et corrigés avant la PR :
  le tout premier `Query` ne partait pas (une socket Tokio neuve refuse `try_send_to`
  tant qu'elle n'a pas reçu son premier événement d'écriture ; seule la répétition à
  1 s passait), et l'adresse basculait à chaque datagramme entre le LAN et le pont
  docker (172.17.0.1).
- **2026-11-25, poste Linux ↔ vm124** (Windows 11, agent jetable compilé en croisé
  `x86_64-pc-windows-gnu`, lancé en SYSTEM, TCP 24901, `C:\LittleBigMouse\v6` intact) :
  la VM démarrée après le poste, chacun voit l'autre (`192.168.4.100:24901` d'un côté,
  `192.168.4.99:24900` de l'autre) ; le `Quit` de l'agent Linux le retire de la liste de
  la VM, et sa relance le fait réapparaître des deux côtés. Un `PeerConnect` vers
  l'adresse lue dans `Discovered` mène chaque côté à `Pending`, avec la bonne empreinte
  (refusé ensuite, rien n'a été appairé). **Le pare-feu de la vm124 était désactivé sur
  les trois profils** pendant l'essai : la règle UDP n'a pas pu être éprouvée en effet.
  Sa commande `netsh`, celle de l'installeur, a été posée puis retirée et se lit
  conforme (entrante, UDP 24817, `LocalSubnet`, programme, tous profils). Reste à voir,
  pare-feu actif, qu'une requête diffusée par une autre machine passe avec la règle et
  pas sans.

### Limites

- IPv4 seulement ; pas de traversée de routeur (la diffusion reste dans le sous-réseau,
  et c'est voulu).
- Le `Bye` part quand le mode est coupé ou l'agent quitté (`Quit`, tray) ; un agent tué
  par un signal (fermeture de session sous Linux) ne le dit pas, et reste listé chez les
  autres jusqu'à « Look again » ou à son prochain démarrage.
- Un agent plus ancien n'annonce rien et ignore les datagrammes : il ne se voit pas et
  ne voit personne. Les deux machines doivent avoir la découverte.
- Les agents jetables d'essai annoncent aussi (port UDP fixe) : ils sont vus des agents
  du réseau qui ont la découverte, pas d'un agent plus ancien.

## macOS

Il n'y a **aucun code macOS** dans le dépôt, et pas seulement pour ce mode :
LittleBigMouse n'a pas de port macOS (ni hook, ni détection d'écrans, ni tray). Ce qui
suit vient de ma connaissance générale des API, sans rien avoir compilé ni exécuté.

Ce qui se porte sans effort : `lbm-net`, `lbm-ipc`, le modèle et la partie « pairs » de
l'agent sont du Rust portable (tokio, rustls).

Ce qu'il faudrait écrire, par rôle :

| Besoin | Esclave | Maître | Piste |
|---|---|---|---|
| Énumération des écrans, EDID, taille physique | Oui | Oui | CoreGraphics ; l'EDID est difficile d'accès sur Apple Silicon |
| Injection pointeur et clavier | Oui | — | `CGEventPost` ; autorisation Accessibilité |
| Table evdev → codes de touches macOS | Oui | Inverse | À écrire |
| Capture souris et clavier | — | Oui | `CGEventTap` ; autorisation Surveillance de l'entrée |
| Mouvement relatif curseur figé | — | Oui | Dissocier souris et curseur |
| Masquage du curseur | Souhaitable | Oui | Pas d'API publique fiable pour une application en arrière-plan |
| Fond d'écran | Optionnel | — | `NSWorkspace` |
| DDC/CI | Optionnel | Optionnel | API privée sur Apple Silicon |

Deux obstacles hors code : les autorisations (Accessibilité, Surveillance de l'entrée)
sont liées à la signature de l'application, donc il faut un binaire signé et notarisé
pour un usage normal ; et sans machine de test, rien de ce qui touche l'entrée ne peut
être validé. Les exécuteurs macOS d'une CI compilent et font tourner les tests
unitaires, mais ne permettent pas d'essayer une capture ou une injection réelle.

Proposition : ne viser d'abord que **macOS esclave**, qui est le rôle le plus court
(écrans + injection), garder le code derrière la CI macOS pour qu'il compile, et
l'annoncer comme non testé en attendant un utilisateur prêt à l'essayer. Le maître
macOS n'a de sens qu'après.

## Ordre proposé

1. Valider l'existant (esclave Linux, VCP 0x60, fonds en direct).
2. ~~Sortir l'automate de traversée du module evdev~~ (fait, #810).
3. ~~Maître Windows~~ (PR #828 ; reste l'essai avec de vrais périphériques et un
   esclave Windows).
4. ~~Corrélation `Loaded`~~ (#809), ~~retour d'erreur vers l'interface~~ (#808), ~~règle
   de pare-feu dans l'installeur~~ (#811).
5. ~~Fenêtres élevées par `StartElevated`~~ (essayé sur la vm124, voir « Esclave
   Windows ») ; ~~service Windows pour le bureau sécurisé et l'utilisateur standard~~
   (#823 à #832) ; reste l'essai de la chaîne complète avec de vrais agents.
6. Presse-papiers : texte, image et fichiers passent sur la paire réelle ; suivent
   `FILEDESCRIPTOR`/`FILECONTENTS` sous Windows, puis le palier 2.
7. Plusieurs esclaves.
8. GNOME, X11, portail `InputCapture`.
9. macOS esclave, si quelqu'un peut l'essayer.

## Travaux en cours

Tenu à jour au fil des branches ; une ligne disparaît quand sa PR est fusionnée et que
les sections ci-dessus en tiennent compte.

Maître Windows : PR #828 (hooks bas niveau), non fusionnée ; l'identité des écrans
distants sur un maître Windows est une PR à part (#827).

La détection des autres outils de partage (PR #814) est fusionnée dans `v6` ; elle est
décrite dans « Ce qui est livré » et « Faiblesses connues ». Les règles graduées sur les
écrans de l'esclave (PR #817) le sont aussi, voir « Rulers on a remote's screens » dans
[`multi-pc.md`](multi-pc.md).

Partout : `lbm-agent/tests/supervise.rs` (`an_absent_hook_is_launched_detached_and_outlives_the_agent`)
échoue en local sur un poste où un vrai hook tourne déjà ; ce n'est pas lié à ces branches.
