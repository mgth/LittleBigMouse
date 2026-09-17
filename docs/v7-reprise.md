# Reprise du frontend egui — 17 septembre 2026

Base : `v6`, `a9fd336e` (correctif du dry-run #725 inclus).
Les notes de conception de `v7-frontend.md` racontent aussi les étapes antérieures ;
leurs listes de manques ne constituent pas toutes un état courant.

## Premier lot : connexion et actualisation

- La fenêtre cherche l'agent en arrière-plan. S'il manque au démarrage, elle lance
  une fois le `lbm-agent` voisin, avec `--ui` pointant vers elle. Construire les deux
  binaires dans le même profil. L'agent garde sa propre durée de vie.
- `LBM_AGENT_ENDPOINT` reste un mode de connexion seule, utile aux essais isolés :
  aucun résident de la session normale n'est lancé si ce socket privé manque.
- Hello vérifie le nom et le protocole avant Subscribe. Connexion, poignée de main
  et écritures ont un délai ; l'attente d'événements après abonnement n'en a pas.
- La connexion se rétablit avec une attente progressive de 250 ms à 5 s. Chaque
  connexion reçoit une nouvelle file de commandes et un nouvel abonnement ; aucune
  commande interrompue n'est rejouée. Une fermeture par le tray ne relance pas l'agent.
- La fermeture de la fenêtre ferme aussi sa connexion et donc son aperçu. Le mode
  Live n'est pas réactivé par une reconnexion.
- Le snapshot v3 reçoit deux champs supplémentaires, compatibles avec le client C# :
  `LayoutRevision` (reconstructions terminées) et `DisplaySignature` (configuration
  qui a produit la reconstruction). Changer d'échelle ou de résolution sans changer
  d'identifiant de layout devient observable par le frontend.
- La carte recharge ses écrans et leurs vignettes hors du fil UI. Une réponse issue
  d'une ancienne détection ne remplace pas une plus récente.
- Une édition non enregistrée reste en mémoire. Sur reconnexion au même bureau,
  identifié par sa signature, elle reste utilisable. Si le bureau a changé, le
  bandeau demande explicitement de l'abandonner pour recharger ; les commandes de
  layout sont désactivées entre-temps, mais Stop reste accessible. Un ancien agent
  sans signature ne permet pas de prouver que le bureau est inchangé.

Vérification : transport contre le vrai agent avec faux hook (aucune souris ouverte),
redémarrage et réabonnement, agent tardif, protocole incompatible, handshake silencieux,
trames fragmentées pendant une écriture, lancement unique, préservation des éditions,
rejet des détections périmées et reconstruction à identifiant constant. La compilation
Windows ne remplace pas un essai sur une session Windows réelle.

Contrôles locaux du lot : 197 tests frontend et 59 tests d'intégration/réconciliation
de l'agent passent ; les six captures de référence sont incluses. `cargo fmt --check`,
Clippy avec avertissements interdits, et `cargo check --all-targets` du frontend pour
`x86_64-pc-windows-msvc` passent. La suite unitaire élargie de l'agent a trois échecs
dans `watch::tests::linux` : création de surveillances inotify refusée avec ENOSPC,
quota utilisateur épuisé sur la machine de validation (56 réussites, 2 tests ignorés).
Ces tests et les limites système n'ont pas été modifiés.

## Deuxième lot : passages et murs dans egui

- `Show crossings` affiche les bandes en carte et dans l'écran agrandi de la liste :
  vert pour un passage, rouge pour un mur. L'infobulle nomme la destination. Le rapport
  utilise le vrai moteur, avec la résistance désactivée pour mesurer les passages.
- `ProbeLayout` reçoit l'identifiant, la révision et le document affiché. L'agent sonde
  une copie et répond en JSON sous l'identifiant de la requête. Aucun enregistrement,
  changement de topologie ou ordre au hook ; un aperçu actif reste intact.
  `Probe` et l'événement XML `Probed` restent compatibles avec Avalonia.
- Le frontend associe les zones aux sources actives, indépendamment des indices et des
  identifiants de moniteurs. Les coordonnées absolues et les bornes inclusives du
  rapport sont converties dans chaque écran ; les bandes restent dans la dalle.
- Après une édition, une attente de 250 ms évite les sondages en rafale. Une seule
  requête est en vol. Les réponses d'anciennes éditions sont consommées sans affichage
  et ne peuvent pas acquitter un Save/Start. Glissement, rechargement du bureau et
  déconnexion retirent les bandes ; le prochain état stable est resondé.
- Un refus est visible et peut être retenté. Un agent sans révision demande une mise
  à jour. Le calcul reste dans l'agent ; la fenêtre reçoit les résultats en arrière-plan.

Vérification : sondage d'une édition avec bouclage pendant un autre aperçu actif,
refus des identifiants/révisions périmés, absence de diffusion aux autres clients,
stabilité du rapport XML existant, des fichiers et des commandes du hook. Les tests
frontend couvrent aussi les réponses retardées, les clics et les infobulles. Deux
nouvelles captures vérifient les bandes dans la carte et la liste, inspectées localement.

Contrôles locaux du deuxième lot : 204 tests frontend et 84 tests d'intégration de
l'agent passent, dont huit captures de référence. Formatage, Clippy avec avertissements
interdits, et compilation de toutes les cibles de test du frontend et de l'agent pour
`x86_64-pc-windows-msvc` passent. La validation sur matériel Windows reste à faire.

## Troisième lot : actions sur les écrans système

- La vue `Displays` liste les sorties connectées, y compris celles désactivées qui ne
  figurent plus sur la carte. Elle propose `Make primary`, `Attach`, `Detach` et une
  actualisation explicite. L'option existante `ShowMonitorActionWarning` gouverne la
  confirmation, qui nomme la sortie et l'effet demandé ; Annuler ne transmet rien.
- Les requêtes v3 additives `DisplayOutputs` et `ChangeOutput` passent par l'agent.
  La seconde porte l'identifiant/révision du layout et la signature de la liste lue.
  L'agent refuse les configurations périmées, layouts étrangers, moteur actif ou
  suspendu et aperçu en cours. La fenêtre demande aussi d'enregistrer ou d'annuler
  ses éditions et propose Stop. Le détachement du dernier écran actif reste interdit,
  même lorsque les confirmations sont désactivées.
- KScreen et XRandR sont les backends disponibles pour ces actions. Avant l'exécution,
  une lecture fraîche vérifie l'identité de la sortie et la signature ; après, une
  nouvelle lecture doit confirmer le résultat. Un retour de processus seul ne vaut
  pas succès. Une erreur reste affichée jusqu'à une nouvelle demande de lecture.
- La reconstruction du layout suit chaque tentative transmise à la plateforme, y
  compris un échec partiellement appliqué. La carte et la liste se rechargent par le
  mécanisme du premier lot. Les anciennes réponses et confirmations sont invalidées ;
  les réponses de ce panneau ne peuvent pas acquitter Save/Start.
- L'inventaire XRandR inclut désormais les sorties connectées sans mode, sans modifier
  l'inventaire historique de la fabrique de layouts. La lecture des positions négatives
  accepte également `1920x1080-1920-1080`, en conservant les fixtures au format `+-1920`.

Limites de ce lot : les commandes Windows n'étaient pas encore portées (voir le sixième
lot ci-dessous). Les opérations Linux sont synchrones et bornées dans l'agent, comme
les opérations de topologie existantes ; elles ne tournent pas sur le fil UI. Aucun
écran réel n'a été détaché pendant la validation : les essais matériels restent à faire
sous KScreen et X11 (principal, détacher/rattacher, dock et profils restaurés).

Contrôles locaux : 211 tests frontend, 70 tests de détection/plateforme et 85 tests
d'intégration de l'agent passent. Les neuf captures de référence incluent le nouveau
panneau et sa confirmation, inspectés localement. Formatage, Clippy sans avertissement
et compilation des cibles de test app/agent pour Windows passent. Le test de transport
utilise le vrai agent et la vérification de topologie avec un compositeur simulé : il
couvre les trois actions, le dernier écran, les requêtes périmées, le moteur actif et
la reconstruction après un échec partiel.

## Quatrième lot : Size et Info

- Les modes `Size` et `Info` sont activés. Le moniteur sélectionné partage un panneau
  de propriétés avec la carte et la liste ; il se place à droite dans une grande
  fenêtre, en dessous dans une fenêtre étroite, avec redimensionnement et défilement.
  Les champs gardent une taille lisible indépendamment du zoom de la carte, suivant
  l'orientation proposée dans [#523](https://github.com/mgth/LittleBigMouse/issues/523).
- `Size` édite largeur et hauteur de dalle, quatre bordures et dimensions extérieures,
  en millimètres dans l'orientation affichée. Les dimensions restent partagées par
  modèle ; les bordures suivent la politique existante, par modèle ou par moniteur.
  La largeur extérieure ajuste les deux bordures latérales à parts égales, la hauteur
  extérieure ajuste celle du bas, puis le layout applique son compactage habituel.
- Le verrouillage des proportions est une aide de session ; les dimensions obtenues
  sont enregistrées. Les valeurs invalides sont refusées avant mutation. Les champs
  restent désactivés pendant une actualisation du bureau ou une action système.
  Les modifications utilisent Save et Live existants et invalident les passages sondés.
- `Info` montre séparément les données de détection et les valeurs du layout édité.
  Les rapports Linux/Windows proviennent de la même détection que le layout, hors du
  fil UI, avant l'application du profil enregistré. Les données absentes sont signalées
  et les anciens résultats suivent le même rejet par génération que les écrans.
- Undo restaure d'abord l'état chargé ou accepté par Save, puis relit le profil.
  Cela corrige le premier lancement sans fichier : les dimensions et les bordures
  modifiées ne restent plus en mémoire après annulation.

Contrôles locaux : 222 tests frontend passent, avec douze captures de référence,
dont les trois nouvelles vues Size large/étroite et Info inspectées visuellement.
Les tests couvrent les quatre rotations, les deux politiques de bordures, les
proportions, les dimensions extérieures, le document SaveLayout et Undo sans fichier.
Formatage, Clippy sans avertissement et compilation des cibles de test du frontend
pour Windows passent. Les captures ont été validées avec le moteur de rendu local ;
la validation de l'application sur matériel Windows reste à faire.

## Cinquième lot : éditeur de sections de résistance

- Le mode `Border resistance` est activé. Les bandes sont dessinées dans la dalle,
  en coordonnées de projection identiques à celles du moteur, sur la carte et dans
  la vue liste. La sélection ouvre les valeurs dans le panneau de propriétés.
- Glisser sur une bande vide crée une section ; ses poignées redimensionnent les
  extrémités et son centre permet de la déplacer. Le double-clic remplit l'espace
  libre ou agrandit la section. Le panneau propose aussi l'ajout, les bornes en mm,
  l'agrandissement et la suppression ; Suppr retire la section sélectionnée hors saisie.
- Les sections ne se chevauchent pas : les gestes s'arrêtent aux voisines et aux
  extrémités du bord, avec 2 mm minimum et 16 sections maximum. L'accrochage suit
  les extrémités, le milieu, les bords visibles des autres écrans et les sections
  parallèles, même éloignées. Ctrl le suspend ; une ligne indique l'alignement atteint.
- Chaque section conserve quatre réglages indépendants : résistance au déplacement,
  blocage du déplacement, résistance au glisser et blocage du glisser. Les couleurs
  et infobulles distinguent les états. Une section nouvelle bloque le déplacement
  et laisse passer le glisser, comme dans Avalonia. Un bord intégralement bloqué dans
  les deux modes affiche l'avertissement existant sur l'absence de contournement Ctrl
  sous Wayland.
- `Mirror` copie les quatre réglages sur le bord opposé de l'écran voisin le plus
  proche dans la distance de parcours, en projetant les coordonnées absolues et en
  conservant le plus grand intervalle libre. L'action est désactivée sans destination.
- L'aperçu du geste reste dans la fenêtre jusqu'au relâchement. Échap, perte de focus,
  changement de vue, de projection ou de données annulent le geste. Une action porte
  l'état du bord sur lequel elle a commencé : une sélection périmée ne peut pas
  modifier une autre section. Save, Live et Undo utilisent les parcours existants.
  Le rapport géométrique `Show crossings` est masqué et suspendu pendant ce mode pour
  que ses bandes ne recouvrent pas celles de résistance.

Contrôles locaux : 240 tests frontend passent, dont les gestes egui, le double-clic,
la saisie numérique, l'annulation, les bords partagés, la copie et le refus d'actions
périmées. Le document SaveLayout est relu puis compilé en liens ; un test du frontend
vérifie aussi le document envoyé à Live et le refus d'édition pendant une actualisation.
Les quinze captures comprennent trois nouvelles références carte/liste/fenêtre étroite,
inspectées localement. Formatage, Clippy sans avertissement et compilation des cibles
de test du frontend pour Windows passent. Les bandes sur les écrans réels et les essais
matériels restent à faire.

## Sixième lot : commandes d'écran sous Windows

- `Displays` utilise désormais le backend Windows pour l'inventaire, `Make primary`,
  `Attach` et `Detach`. Les gardes de l'agent et les confirmations du frontend restent
  les mêmes : moteur arrêté, pas d'aperçu, bon layout/révision et inventaire récent.
- L'inventaire CCD comprend les cibles connectées mais désactivées. Les chemins de
  liaison possibles sont regroupés par cible ; le connecteur transmis par l'API est
  le chemin d'interface du moniteur, jamais le nom recyclable `DISPLAYn`. Attacher
  choisit une source libre pour cette cible précise, sans remplacer un autre écran
  ni créer implicitement un clone. Windows choisit son mode et son placement initiaux.
- Le changement de principal translate toutes les sources actives, une fois par
  source, et place la source choisie à `(0,0)`. Résolutions, formats et modes cibles
  sont conservés. Lors du détachement du principal, une source restante devient
  l'origine. Le dernier moniteur actif ne peut pas être détaché, y compris en présence
  de chemins candidats multiples ou d'écrans clonés.
- Les trois actions utilisent une seule application de configuration CCD. Il n'y a
  pas d'écritures DEVMODE en attente dans le registre. `SetDisplayConfig` valide
  d'abord le plan sans le modifier ; une nouvelle lecture doit ensuite retrouver la
  signature attendue avant l'application et l'enregistrement dans la base Windows.
  Le changement de principal n'autorise pas l'ajustement automatique des modes ;
  l'attachement et le détachement autorisent les ajustements nécessaires au pilote.
- Une relecture suit même un échec du pilote. Un retour réussi seul ne suffit pas :
  le moniteur demandé doit avoir l'état attendu, les autres doivent conserver leur
  état d'activation et un changement de principal doit conserver modes sources et
  positions relatives. Toute divergence est signalée ; la reconstruction habituelle
  du layout suit la tentative. Les courses de taille des buffers CCD sont retentées
  trois fois avant de signaler un bureau instable.

Références API : [position de la source principale](https://learn.microsoft.com/en-us/windows/win32/api/wingdi/ns-wingdi-displayconfig_source_mode),
[validation et application CCD](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setdisplayconfig),
[énumération des chemins et courses de buffers](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-querydisplayconfig).

Contrôles locaux : 87 tests de détection/plateforme, 85 tests d'intégration de l'agent
et un test du document IPC Windows passent. Les 17 nouveaux tests de planification
et de transaction tournent sous Linux avec un backend simulé : identités, sources
occupées, clones, dernier écran, translation, refus avant écriture, course pendant la
validation et vérification après application. Clippy sans avertissement passe sur
Linux et pour `x86_64-pc-windows-msvc`, sur les cibles de test display/agent/app.
Deux tests supplémentaires de matérialisation des structures Win32 sont compilés
pour Windows, mais n'ont pas été exécutés ici.

Validation matérielle encore nécessaire sous Windows : écran principal à gauche/en
portrait, détachement du principal, plusieurs moniteurs désactivés de même modèle,
rattachement après dock, clone, fréquences et DPI mixtes, refus du pilote et session
distante. Aucun appel de modification Win32 n'a été exécuté depuis cette machine Linux.

## Septième lot : placement global Windows

- `Apply topology` relie le solveur millimètres → pixels aux chemins CCD des écrans
  actifs. Les interfaces de moniteurs servent d'identités ; les noms `DISPLAYn` ne
  servent pas à choisir une cible. Le plan couvre tous les écrans actifs, conserve
  les chemins, les modes et le principal à l'origine. Les clones partagent une seule
  position ; des positions contradictoires sont refusées.
- Le document est appliqué à une copie du layout. L'aperçu calcule les déplacements,
  valide les changements proposés avec Windows et restitue les coordonnées avant/après.
  Il ne sauvegarde rien, ne modifie pas le layout courant et n'appelle pas
  `SetDisplayConfig(APPLY)`. Une disposition déjà satisfaite évite les appels CCD
  de validation/application, mais un Apply réel sauvegarde tout de même les éditions
  physiques du profil.
- Les bornes détectées sont comparées à une lecture CCD fraîche. Les coordonnées
  non finies/hors plage, les identités ambiguës et les configurations incomplètes
  sont refusées. La signature est relue après validation, puis après sauvegarde ;
  une divergence empêche l'application. Une erreur de sauvegarde empêche également
  toute application CCD. Le profil est écrit avant le changement système pour
  survivre à la reconstruction du layout.
- Une seule transaction CCD applique et persiste les positions, sans autoriser
  Windows à ajuster automatiquement les modes. Une relecture vérifie les chemins,
  l'état des moniteurs, le principal, les positions et les modes sources. Elle suit
  aussi un échec du pilote. L'agent déclenche la reconstruction après une tentative
  réelle, y compris en erreur ; les aperçus ne la déclenchent jamais.
- Le frontend envoie `LayoutRevision` avec Apply et son aperçu. Windows exige une
  révision courante et un moteur arrêté, sans aperçu ni suspension. Les anciens
  clients Linux restent compatibles avec le champ absent. Le document transmis ne
  peut pas rétablir un ancien état Enabled après Stop. Le menu Windows explique
  l'arrêt nécessaire et garde les échelles actuelles ; `AdjustScale=true` est refusé
  par l'agent Windows.

Contrôles locaux : 95 tests de plateforme, 86 tests d'intégration de l'agent,
3 tests du raccordement layout/Windows et 68 tests de l'application passent.
Les nouveaux scénarios couvrent la copie du document, le maintien de Stop, les
révisions périmées/absentes, les clones, les coordonnées négatives, les courses de
configuration, les refus du pilote/de sauvegarde et les résultats partiels.
Formatage, Clippy sans avertissement et compilation de toutes les cibles de test
display/agent/app passent sur Linux et pour `x86_64-pc-windows-msvc`.

Les appels Win32 restent à qualifier sur une session Windows réelle : écrans à
gauche/au-dessus, portrait, fréquences et DPI mixtes, clone, dock pendant validation,
refus de dispositions avec trou/recouvrement. Les tests locaux utilisent des backends
simulés ; aucun écran réel n'a été déplacé. Un profil sauvegardé avant un refus du
pilote reste sauvegardé, même si Windows n'a pas accepté le placement.

## Huitième lot : règles et bandes sur les écrans Windows

- `lbm-overlay` fournit maintenant une bibliothèque utilisée par egui. `Rulers on
  screens` projette les graduations du moniteur sélectionné sur les quatre bords
  de chaque écran attaché. `Resistance on screens`, dans le mode résistance,
  affiche les sections éditées avec les mêmes couleurs que la carte. Les deux
  affichages sont exclusifs et désactivés par défaut.
- Les origines et longueurs des règles viennent des projections physiques du
  layout. Chaque écran utilise son propre rapport pixels/mm ; le facteur DPI ne
  multiplie pas une seconde fois les graduations. Il dimensionne les bandes
  (100 DIP pour les règles, 40 pour les sections). Les positions négatives, les
  dalles en portrait et les décalages du zéro sur les autres écrans sont couverts.
  Les marques extérieures au moniteur mesuré sont atténuées. Les coins sont coupés
  en diagonale pour que deux bandes voisines ne se recouvrent pas.
- Le rendu CPU produit du BGRA prémultiplié, avec graduations au millimètre,
  subdivisions et chiffres de mesure déterministes. La scène est recalculée sur
  une copie du layout pendant les gestes de position/résistance, sans commit ni
  demande Save/Live. Les rafraîchissements sont limités à 20 Hz ; une scène inchangée
  ne réalloue pas les fenêtres et ne redessine pas les surfaces natives.
- Sous Windows, les fenêtres superposées sont sans activation, hors barre des tâches,
  au premier plan et transparentes aux clics. Elles appartiennent à la fenêtre egui,
  donc suivent sa réduction et sa fermeture. Leur création et mise à jour utilisent
  un contexte DPI par moniteur V2, restauré ensuite sur le fil UI. Les bornes du
  moniteur, son DPI et les dimensions réellement accordées sont vérifiés.
- Une déconnexion, un changement de page/mode, une révision de bureau périmée, la
  suspension, Échap ou un layout étranger arrêtent l'affichage. Les notifications
  natives de changement d'écran/DPI masquent les fenêtres et demandent de recharger.
  Une erreur après une création partielle détruit toutes les surfaces ; les ressources
  HWND, DC et bitmaps sont libérées. Les overlays n'écrivent aucun profil et ne
  déplacent aucun écran système.

Contrôles locaux : 69 tests de l'application, 10 tests d'overlay et 11 tests de gestes
de résistance passent. Le raccord des gestes expose la prévisualisation avant
relâchement et la retire après annulation ; le cycle de vie de l'application couvre
déconnexion, bureau périmé, layout étranger, suspension, changement de page, réduction
et Échap. Une image de référence du rendu CPU est vérifiée pixel par pixel et a été
inspectée visuellement : `rust/crates/lbm-overlay/tests/snapshots/screen-overlays.png`.
Formatage et Clippy sans avertissement passent sur Linux et sur toutes les cibles de
test overlay/ui/app pour `x86_64-pc-windows-msvc`.

Références de comportement natif : [fenêtres layered et transparence aux clics](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features),
[contexte DPI du fil et des fenêtres](https://learn.microsoft.com/en-us/windows/win32/hidpi/high-dpi-improvements-for-desktop-applications).

Limites restantes : les fenêtres Win32 sont compilées, pas exécutées sur cette
machine Linux. Qualifier sur Windows le clic traversant vers une autre application,
l'absence de vol du focus, les échelles 100/150/200 %, la réduction/fermeture, les
changements de DPI, le dock et les jeux plein écran. Les bureaux clonés sont refusés
pour éviter des dessins contradictoires sur les mêmes pixels. Le backend Linux
reste le prototype autonome : il n'est pas exposé comme fonction opérationnelle
dans egui. Les lignes de référence traversant les écrans pendant l'accrochage des
sections restent à raccorder.

## Suite, dans l'ordre

1. Raccorder les surfaces natives Linux (Wayland layer-shell/X11) à la bibliothèque
   d'overlay, puis les lignes de référence des sections. Qualifier les overlays et
   les changements de topologie sur matériel Windows à DPI mixtes.
2. Porter l'éditeur de fond d'écran et `IDesktopWallpaper`, puis qualifier le premier
   périmètre egui sur matériel (veille, dock, UAC, reprise, démarrage et mémoire).
3. VCP/mires, calibration, télécommandes, import/export et mise à jour ; bascule du
   packaging et retrait de .NET une fois les fonctions conservées remplacées.

Essais manuels encore nécessaires : lancement installé sans agent, agent élevé sous
Windows, débranchement avec édition en cours, changement de résolution/DPI, fermeture
pendant un aperçu et ouverture depuis le tray. Ne pas lancer l'agent de développement
sur les données réelles uniquement pour valider le transport.
