# Mode multi-PC : état du travail et reste à faire

État au 2026-11-24, branche `v6` (`cf2bc5c7`, suivi jusqu’à `d7204166`), après la fusion des PR #800 (le mode)
et #801 (fonds d'écran suivis en direct).

Mis à jour au fil des travaux (dernière fois le 2026-11-24, règles sur l'esclave) : voir « Travaux en cours » à la fin.

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
| Hook maître | `lbm-hook/src/hook/crossing` (règles, toutes plateformes), `lbm-hook/src/hook/linux/evdev` (seul client) | Traversée, capture des claviers quand le pointeur est parti, retour local si le hook maître perd son relais, curseur local masqué |
| Hook esclave | `lbm-hook/src/remote` | Injection `SendInput` (Windows) et uinput (Linux), relâchement de tout ce qui est tenu en fin de session, curseur rangé |
| Interface | `lbm-app` | Page Machines, écrans distants sur la carte, bascule de source, vignettes de fond d'écran distantes |
| Commutation d'entrée | `lbm-agent/src/input_switch.rs` | VCP 0x60 par `ddcutil` ou dxva2, pour un moniteur câblé sur deux entrées sans KVM |
| Fonds d'écran | `lbm-agent/src/peers/wallpapers.rs`, `wallpaper_watch.rs` | Vignettes envoyées par l'esclave, suivies par événements (inotify Plasma, registre et diffusion Windows) |
| Confort (PR #813) | `hook::crossing`, `lbm-agent/src/peers`, tray, `lbm-app` | Indicateur de la machine qui a la souris et le clavier (tray, page Machines) ; Verr. Maj/Num/Défil de l'esclave alignés sur ceux du maître à l'`Enter` ; pointeur verrouillé sur une machine par `PointerLockShortcut` (`multi-pc.json`, aucun par défaut), toujours libéré par le secours, la fin du flux, l'arrêt du maître ou le changement de client |
| Règles sur l'esclave (cette branche) | `lbm-app/src/remote_rulers.rs`, `lbm-agent/src/peers/overlay.rs`, `lbm-overlay/src/standalone.rs` | « Rulers on screens » dessine aussi les règles sur les écrans de l'esclave : calculées par le maître en mm et dans les pixels de l'esclave, envoyées par `Overlay` (capacité `RemoteOverlay`), contrôlées puis dessinées par l'esclave, inertes (le clic traverse), retirées à la chute du lien, à la fermeture de l'UI du maître, à la coupure du mode et au changement d'écrans |

Règle de sûreté visée : **quand un maillon du flux tombe, le pointeur est chez
lui**. Le maître ramène son pointeur et relâche ses claviers ; l'esclave relâche toutes
les touches et tous les boutons que le flux a laissés enfoncés. Le raccourci de secours
reste reconnu pendant que le clavier est capturé. Le hook de l'esclave qui perd seul
sa connexion à son agent en pleine session y est soumis aussi, comme une traversée qui
ne trouve pas de hook chez l'esclave : l'agent de l'esclave dit `InputReset` et le
maître ramène son pointeur.

## Ce qui a été essayé pour de bon

Une seule paire complète : **maître Linux (KDE Plasma 6, Wayland, backend evdev) →
esclave Windows 11**, avec un écran partagé par KVM. L'esclave Linux et les fonctions
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
| Règle de pare-feu 24817 posée et retirée par l'installeur | Vérifié sur la vm124 (PR #811) |
| Règles sur l'esclave Windows (vm124, 4K à 1,5 + 1080p) : affichées à leur place sur les deux écrans, zone mesurée qui suit l'écran choisi, mise à jour, retirées sur ordre et à la chute brutale du lien (maître tué), pointeur qui les traverse (`WindowFromPoint` rend la fenêtre du dessous) | Vérifié avec `peer_drive` comme maître et `peer_remote` comme esclave (l'agent installé du mainteneur tournait à côté, intact) |
| Règles sur un esclave Linux : surfaces layer-shell, mise à jour, bail renouvelé, hôte d'overlay disparu à la fin | Vérifié dans un KWin imbriqué sans écran (`kwin_wayland --virtual`), pas sur un vrai bureau |

Jamais essayé sur du vrai matériel, bien que codé et couvert par des tests :

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
- les trois fonctions de la PR #813 : l'indicateur, l'alignement des verrous (`GetKeyState`
  depuis le fil de l'injecteur Windows peut retarder sur l'état système ; un esclave
  Linux dont le hook a capturé les claviers lit des LED que le compositeur ne peut plus
  écrire ; délai entre la frappe injectée et la LED) et le pointeur verrouillé sous un
  vrai compositeur (verrouillé chez soi, le raccourci atteint aussi le bureau).

## Faiblesses connues

- Fonds en mode « Span » sous Linux à une échelle différente de 1 : mal positionnés, y
  compris dans l'aperçu local.
- Retour d'erreur vers l'interface (#808) : un esclave dont le lien tombe avant de
  répondre à une commutation d'entrée, ou un esclave plus ancien, ne laisse aucun échec
  chez le maître ; l'échec d'une connexion à une adresse tapée n'apparaît que comme
  l'erreur de cette requête. L'interface elle-même n'a été vue que par les tests
  kittest, jamais à l'écran.
- Règles sur l'esclave : elles ne se tirent pas (inertes là-bas, on aligne en tirant
  celles des écrans du maître) ; « Rulers on screens » reste grisé quand l'écran
  sélectionné est celui de l'esclave, comme avant ; sous Linux, l'esclave renouvelle le
  bail de l'hôte d'overlay toutes les 500 ms tant que des règles sont affichées (pas
  au-delà).
- Le premier lien avec un esclave qui apporte des écrans inconnus crée un profil
  **désactivé** : le moteur s'arrête jusqu'à ce que l'utilisateur l'active.
- Esclave Linux : si son propre backend evdev route, une souris locale reprend d'où ce
  backend avait laissé le curseur, pas d'où le maître l'a laissé. Deux autres défauts
  trouvés à l'essai sont corrigés (PR #807) : un esclave
  dont le moteur local est arrêté n'injectait pas le pointeur (il ne connaissait son
  bureau que par un `Load`), et KWin 6 jette ce qu'un périphérique uinput émet dans ses
  ~300 premières millisecondes, d'où un premier `Enter` sans effet.

## Matrice de support

| | Maître | Esclave |
|---|---|---|
| **Linux, backend evdev** | Livré, vérifié sous KDE Wayland | Livré, vérifié sous KWin 6 avec `peer_drive` comme maître (correctifs de la PR #807) |
| **Linux, backend portail Wayland ou X11** | Absent : les zones distantes ne sont jamais franchissables | Livré si `/dev/uinput` est accessible en écriture, jamais essayé sur matériel |
| **Windows** | Absent | Livré, vérifié |
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
| Curseur masqué, côté maître | Wayland avec layer-shell (KWin vérifié), X11 par XFixes ; rien sous GNOME, qui n'a pas layer-shell | Sans objet tant qu'il n'y a pas de maître Windows |
| Curseur rangé, côté esclave | Rien de visible | Oui |
| DDC/CI | `ddcutil` | dxva2 |
| Profil conservé à travers un KVM | Oui si la dalle a une clé commune aux deux machines **et unique parmi les écrans** (l'id du moniteur est alors cette clé). Sans numéro de série dans l'EDID, ou si le KVM le masque, l'écran distant prend un id propre à sa source et la bascule sélectionne un autre profil. Avec deux dalles annonçant le même numéro de série, celle que le KVM bascule est rattachée à sa jumelle restée sur le maître, et le profil change aussi | Non : Windows nomme ses moniteurs autrement |

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

C'est le gros morceau, et celui qui rend le mode symétrique. Rien n'existe : le hook
Windows actuel ne connaît pas les zones distantes.

- **Mouvement relatif pendant que le pointeur est parti.** Le hook bas niveau de
  Windows donne des positions absolues bornées au bureau : au bord, plus rien ne bouge.
  Il faut lire les déplacements bruts (Raw Input) ou recentrer le curseur à chaque
  rapport. À trancher par un essai.
- **Capture du clavier** : hook clavier bas niveau qui avale les touches, et la table
  inverse de `remote/keymap.rs` (scancode Windows → code evdev). Ctrl+Alt+Suppr et
  quelques raccourcis système ne peuvent pas être capturés.
- **Curseur masqué** : la fenêtre d'un pixel de l'esclave Windows (`remote/windows/hider.rs`)
  est probablement réutilisable telle quelle.
- **Automate de traversée** : fait (PR #810). Les règles vivent dans `hook::crossing`.
  Un maître Windows implémente le trait `Platform` (flux, claviers, curseur, pointeur)
  et appelle `locate` après chaque mouvement routé. Points ouverts pour lui : pas de
  capture par périphérique, donc avaler les touches dans le hook clavier bas niveau
  sauf les relâchements de touches déjà enfoncées à l'`Enter` ; un « frame » de hook
  bas niveau est un seul événement, laissé passer ou avalé.
- **Identité des moniteurs** : nommer un écran distant par sa clé de dalle ne suffit pas
  sous Windows, dont les ids locaux ont une autre forme. Sans cela, un KVM crée un
  nouveau profil à chaque bascule.
- **Fenêtres élevées** : à vérifier — un hook bas niveau d'un processus non élevé ne
  reçoit vraisemblablement pas l'entrée destinée à une fenêtre élevée. `StartElevated`
  (voir plus bas) est la piste existante.

### 3. Esclave Windows : les deux trous

- **Fenêtres élevées** : mouvements, clics et touches passent tous par le même
  `SendInput`, soumis à UIPI : depuis un hook non élevé, rien n'est garanti quand une
  fenêtre élevée a le focus. Une option existe déjà : `StartElevated` lance, pour un
  administrateur à jeton fractionné, un agent élevé dont le hook hérite du niveau
  d'intégrité (`lbm-agent/src/elevation.rs`). **Essayé sur la vm124** : depuis un agent
  non élevé, rien ne passe, pas même le mouvement ; depuis un agent élevé, tout passe.
  Aucun code de plus n'est nécessaire pour un administrateur, hormis un correctif :
  l'agent lisait `StartElevated` dans la configuration réelle et non dans celle de
  `--config-dir`, et les chemins relatifs n'étaient pas rendus absolus avant la relance élevée (PR #811). Le refus de l'invite UAC n'a pas été observé. Un utilisateur standard reste sans solution hors
  service Windows.
- **Bureau sécurisé** (UAC, écran de verrouillage, Ctrl+Alt+Suppr) : hors d'atteinte
  d'un processus de session. Il faut un service. C'est ce qui a bloqué un essai pendant
  le développement : la machine était verrouillée.

Le bureau sécurisé est un chantier à part, avec ses propres questions de sécurité ; il
couvrirait aussi les fenêtres élevées pour un utilisateur standard.

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
- **Presse-papiers** : rien n'existe ; voir la section suivante. Le glisser-déposer
  viendrait après.
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

Rien n'existe. Objectif minimal : **texte et image** dans les deux sens ; le reste par
paliers. Ce qui suit est une proposition, tirée de ma connaissance des API et de ce que
font Deskflow et consorts — rien n'a été essayé.

### Formats, par palier

| Palier | Contenu | Windows | Linux | Remarque |
|---|---|---|---|---|
| 1 | Texte | `CF_UNICODETEXT` | `text/plain;charset=utf-8` | Convertir les fins de ligne |
| 1 | Image | `CF_DIBV5`, et le format enregistré `PNG` | `image/png` | PNG sur le fil : sans perte, transparence conservée |
| 2 | Texte mis en forme | `HTML Format`, `Rich Text Format` | `text/html`, `text/rtf` | L'en-tête de `HTML Format` (décalages) est à reconstruire |
| 3 | Fichiers | `CF_HDROP` | `text/uri-list` | Ce n'est plus du presse-papiers mais un transfert de fichiers : chemins à réécrire, flux, annulation |
| 4 | Glisser-déposer entre machines | — | — | Dépend du palier 3 |

Au palier 1, proposer aussi le texte brut quand la source n'offre que du HTML : c'est ce
que fait toute application et cela couvre la plupart des collages.

### Transfert à la demande

Ne pas envoyer le contenu à chaque copie : une capture d'écran 4K copiée puis jamais
lue ailleurs n'a pas à traverser le réseau.

- À chaque changement, la machine qui possède le presse-papiers **annonce** les formats
  disponibles (un message de contrôle, quelques octets).
- L'autre machine se déclare propriétaire d'un presse-papiers **sans contenu** : rendu
  différé sous Windows, source de données qui n'écrit qu'à la demande sous Wayland et
  X11.
- Le contenu n'est demandé qu'à la **première lecture** d'un format par une application,
  dans ce format. Ce n'est pas forcément un collage de l'utilisateur : sous Windows, un
  gestionnaire d'historique du presse-papiers peut lire dès l'annonce, ce qui déclenche
  le rendu et donc le transfert. La garantie est « à la demande d'une application ».
  S'il faut attendre un geste de l'utilisateur, il faut un déclencheur explicite
  (raccourci, ou annonce faite seulement au passage du bord).

Conséquences sur le protocole : un message d'annonce, une requête et une réponse ; la
réponse dépasse la borne de 1 Mio des trames de contrôle, donc un nouveau type de trame
découpée, qui ne doit jamais passer devant le flux d'entrée (même règle que les fonds
d'écran). Une capacité `Clipboard` dans `Hello`, pour qu'un pair plus ancien l'ignore.

### Écouter et écrire, par système

| Système | Écouter les changements | Écrire | Où |
|---|---|---|---|
| Windows | `AddClipboardFormatListener` (événement, pas de scrutation) | Rendu différé (`WM_RENDERFORMAT`) | La fenêtre cachée que l'agent a déjà |
| Wayland | Protocole `ext-data-control-v1`, ou `wlr-data-control` | Source de données du même protocole | Une connexion Wayland de l'agent ; support selon le compositeur, à vérifier pour KWin, absent de GNOME |
| X11 | XFixes (`XFixesSelectionNotify`) | Propriétaire de la sélection `CLIPBOARD` | Agent |

Conforme à la règle du projet : uniquement des événements, et rien d'installé tant
qu'aucun pair n'est lié ou que l'option est coupée.

### Garde-fous

- **Option coupée par défaut**, par machine, et par sens (recevoir, envoyer).
- **Secrets** : ne pas annoncer ce qu'un gestionnaire de mots de passe marque comme
  sensible (`ExcludeClipboardContentFromMonitorProcessing` sous Windows,
  `x-kde-passwordManagerHint` sous KDE).
- **Taille** : une borne configurable ; au-delà, l'annonce n'est pas faite et l'interface
  le dit.
- **Sélection primaire** de Linux (clic milieu) : hors périmètre, elle n'a pas
  d'équivalent Windows.
- Ne jamais réannoncer ce qu'on vient de recevoir : sinon les deux machines se
  renvoient le presse-papiers sans fin.
- Le contenu ne circule que sur le lien TLS épinglé, entre machines appairées.

### Ce qu'il faut trancher

- Le presse-papiers suit-il le pointeur (comme Deskflow, synchronisé au passage du
  bord) ou chaque copie est-elle annoncée tout de suite ? L'annonce immédiate avec
  transfert à la demande est plus simple à expliquer et ne coûte presque rien.
- Qui porte le code sous Linux : l'agent n'a pas de connexion Wayland aujourd'hui, le
  hook en a une pour masquer le curseur. Le presse-papiers n'a rien à faire dans le
  chemin de l'entrée ; je le mettrais dans l'agent.

## Fonctionnalités à envisager

Des idées, pas des engagements. Les points 1, 3 et 4 sont livrés (PR #813), jamais
essayés sur matériel ; les autres ne sont pas commencés. Dans l'ordre d'utilité estimée.

1. **Verrouiller le pointeur sur une machine** par un raccourci *(livré, PR #813)*. Pour les jeux et le
   plein écran, où l'on franchit le bord par accident. La reconnaissance d'un raccourci
   dans le flux capturé existe déjà pour le raccourci de secours.
2. **Raccourcis pour sauter d'une machine à l'autre** sans viser un bord, avec retour à
   la dernière position. Utile quand les écrans ne sont pas côte à côte.
3. **Synchroniser les verrous** (Verr. Maj, Verr. Num) à l'entrée sur l'esclave *(livré, PR #813)*.
4. **Indicateur de machine pilotée** *(livré, PR #813)* dans le tray et dans l'état de l'agent :
   aujourd'hui seul le curseur dit où va le clavier.
5. **Réveil de l'esclave** : Wake-on-LAN à l'approche du bord, et pas de mise en veille
   de l'écran de l'esclave pendant qu'on le pilote. Le WoL existe déjà dans le dépôt
   pour les téléviseurs.
6. **Verrouiller les deux sessions ensemble** : verrouiller le maître verrouille
   l'esclave. Le déverrouillage à distance dépend du service Windows.
7. **Touches système vers l'esclave** : choisir si Alt+Tab, la touche Windows et
   consorts vont à la machine pilotée ou restent au maître.

Moins prioritaires :

- **Découverte automatique** des machines sur le réseau local.
- **Suivre l'entrée du moniteur** : quand un écran partagé change d'entrée par ses
  propres boutons, basculer la source active. Seulement si le moniteur le signale par
  un événement, le projet excluant la scrutation.
- **Molette et pointeur aux réglages de l'esclave** : vitesse et sens de défilement
  viennent du maître aujourd'hui.

Écartés : le partage audio et le transfert de fichiers autonome, hors du sujet de
LittleBigMouse.

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
3. Maître Windows.
4. ~~Corrélation `Loaded`~~ (#809), ~~retour d'erreur vers l'interface~~ (#808), ~~règle
   de pare-feu dans l'installeur~~ (#811).
5. ~~Fenêtres élevées par `StartElevated`~~ (essayé sur la vm124, voir « Esclave
   Windows ») ; reste le service Windows pour le bureau sécurisé.
6. Presse-papiers, palier 1 (texte et image), puis palier 2.
7. Plusieurs esclaves.
8. GNOME, X11, portail `InputCapture`.
9. macOS esclave, si quelqu'un peut l'essayer.

## Travaux en cours

Tenu à jour au fil des branches ; une ligne disparaît quand sa PR est fusionnée et que
les sections ci-dessus en tiennent compte.

- `feat/mpc-remote-rulers` : les règles graduées sur les écrans de l'esclave (commande
  `Overlay`, capacité `RemoteOverlay`, requête `PeerOverlay`), voir « Rulers on a
  remote's screens » dans [`multi-pc.md`](multi-pc.md).

Partout : `lbm-agent/tests/supervise.rs` (`an_absent_hook_is_launched_detached_and_outlives_the_agent`)
échoue en local sur un poste où un vrai hook tourne déjà ; ce n'est pas lié à ces branches.
