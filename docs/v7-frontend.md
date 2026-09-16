# Frontend Rust — notes de conception

Complément de la phase 6 du plan (`docs/v6-architecture-plan.md`). Ce document décrit ce que les
spikes ont établi et ce qu'ils ont laissé ouvert ; le plan reste la référence pour le périmètre.

Quatre crates, séparés parce qu'ils répondent à des questions différentes et se testent
différemment :

| crate | ce qu'il porte | comment il se vérifie |
|---|---|---|
| `lbm-ui` | l'état, les `Action`, l'`update` pur, les vues | `egui_kittest`, sans fenêtre |
| `lbm-icons` | les alias de logos, la recoloration | rendu resvg, sans fenêtre |
| `lbm-overlay` | les surfaces des règles et des bandes | à l'écran, mesuré sur capture |
| `lbm-layout` (existant) | le modèle que la carte dessine | déjà épinglé par l'oracle |

## L'interface se pilote sans serveur graphique

C'est le premier acquis, et il conditionne tout le reste. `egui_kittest` pilote les widgets par
**l'arbre d'accessibilité** : pas de fenêtre, pas de GPU, pas de compositeur, dans le `cargo test`
ordinaire. L'UI Avalonia n'a jamais été vérifiable ainsi sur une machine Wayland — `xdotool` ne
clique pas sous KWin — et chaque contrôle d'interface y était une capture d'écran et un
plissement d'yeux.

API utile : `Harness::new_ui(|ui| …)`, `harness.get_by_label("Start").click()`, `harness.run()`,
et **`node.accesskit_node().is_disabled()`** — ni `is_enabled` ni `is_disabled` directement sur le
`Node` de kittest.

**Piège, payé une fois** : placer une étiquette avec `ui.put(rect, …)` fait mesurer au harnais le
rectangle **passé**, pas celui du texte. Un test affirmant « le nom suit le cadre » restait vert
avec une police figée. Pour qu'un test de taille morde, l'étiquette doit se dimensionner
elle-même : `ui.scope_builder(UiBuilder::new().max_rect(…))` puis `ui.add(label)`.

## Les surfaces superposées : deux serveurs, deux repères

- **Wayland** interdit à un client de se positionner. `zwlr_layer_shell_v1` (KWin l'annonce en
  v5) est la seule issue : couche `Overlay`, **ancrage à deux bords adjacents**, et les marges
  deviennent une position.
- **X11** n'interdit rien : fenêtre **override-redirect** (hors du gestionnaire de fenêtres,
  placée exactement, pas d'entrée dans la barre des tâches), **visuel 32 bits** pour la
  transparence, **forme d'entrée vide** pour laisser passer le pointeur.

**Le repère n'est pas le même, et c'est ce qui compte pour une règle** : Wayland place en pixels
**logiques** de la sortie (3072 pour un écran 3840 à l'échelle 1,25), X11 en pixels **physiques**
du serveur. Se tromper de repère, c'est se tromper de 25 % sur une machine à 1,25.

X11 vérifie mieux que Wayland : `GetGeometry`, `GetWindowAttributes` et `ShapeGetRectangles` font
**rendre compte au serveur**. Le protocole Wayland ne permet pas de relire l'état d'une surface,
donc la traversée du pointeur y reste déclarée et non prouvée.

**Piège d'outillage** : `blocking_dispatch` attend un événement qu'une surface tranquille ne
reçoit jamais. Un outil qui affiche quelque chose chez quelqu'un doit **sonder**, jamais bloquer,
sous peine de laisser une bande collée à l'écran.

## Les icônes

La table des logos de fabricants n'est écrite **nulle part ailleurs que dans les noms de
fichiers** : chaque jeton séparé par un point est une clé, donc `Acer.CHE.ALI.ACR.API.svg` répond
à cinq noms. Perdre la règle, c'est perdre la table.

Deux défauts de l'existant, non repris :

- `IconService` ne substitue que les orthographes **entre guillemets** (`fill="#000000"`), alors
  qu'Inkscape écrit `style="fill:#000000;…"` — ces icônes ne sont **jamais recolorées** dans l'app
  livrée, dont le logo LittleBigMouse lui-même ;
- `IconView.EffectiveForeground` transforme tout noir pur en blanc en thème sombre, ce qui rend
  les graduations de l'équerre blanches sur blanc. Ici **la couleur demandée est la couleur
  obtenue**.

## Les règles de la barre du bas, reprises du C#

De `LocationControlViewModel`, et plus riches qu'il n'y paraît :

- **Start** n'est pas « le moteur est arrêté ». C'est ainsi qu'une **édition atteint le moteur**,
  donc un moteur qui tourne avec des modifications non enregistrées propose Start ; et un layout
  étranger peut toujours être renvoyé, puisque l'envoyer simule au lieu de router.
- **Stop** s'applique aussi à un moteur **en pause** : la pause est le moteur qui s'efface devant
  une application exclue, et le suiveur C# ne bouge même pas son drapeau `Running`.
- **Save et Undo** sont une condition lue deux fois, et **aucune ne dépend du moteur**. Perdre une
  édition parce qu'un daemon manque serait le pire des deux mondes.

## La carte, et trois écarts assumés au C#

`lbm-ui::map` répond à deux questions et pas une de plus : **combien vaut un millimètre
ici**, et **où est le coin du bureau**. Tout le reste suit par `frame`. Un rapport et non
deux : Avalonia porte une paire `VisualRatio` mais `FrameMover.cs:167` écrit
`var ratioY = ratioX;` — la paire est une forme que le code n'emploie jamais.

Trois choses ne sont **pas** reprises, chacune annotée là où elle s'applique :

- **Le trou du garde vide.** `GetRatio` se protège par `all.Width * all.Height > 0.0`
  avant de diviser. Ce test est **vrai** pour `Rect.Empty`, dont la largeur et la hauteur
  valent toutes deux −∞ : le produit vaut +∞, le garde laisse passer, et
  `min(w / −∞, h / −∞)` donne un rapport de **moins zéro** — la carte s'évanouit au lieu
  de retomber sur 1:1. `PhysicalBounds` atteint `Empty` par le quirk d'union que ce dépôt
  porte fidèlement (vide dès qu'un opérande l'est), donc **un seul écran non mesuré
  suffit**. La règle que le garde visait est « une étendue par laquelle je peux diviser ».
- **Le jeu non centré.** `Math.Min` fait des bandes noires, et l'AXAML demande que le
  résultat soit centré, mais les cadres vivent dans un `Canvas`, qui arrange à
  `Canvas.Left/Top` et ignore l'alignement — dans l'app livrée, tout le jeu tombe à droite
  et en bas.
- **La sélection au pixel exact.** `FrameMover.cs:111-116` ne sélectionne que si le point
  de relâchement **égale** celui de l'appui, sur des doubles. Un dixième de pixel de
  dérive — ce qu'un pavé tactile produit en permanence — en fait un glissement de longueur
  nulle, et l'écran n'est jamais sélectionné.

**Le mode liste n'a demandé aucune seconde fonction** : c'est le même ajustement sur une
autre étendue (l'écran sélectionné au lieu de tous), et son cas « rien de sélectionné »
est `Rect::EMPTY`, qui retombe sur le même repli.

Et un défaut **à moi**, trouvé en me relisant et non par un test : le centrage calculait
l'étendue dessinée depuis l'extent, si bien que le repli plaçait la carte **à l'infini**.
Le cas n'était pas théorique — c'est exactement celui du premier point, écrans présents et
union vide, où chaque cadre partait à l'infini au lieu d'être seulement à la mauvaise
échelle.

**Les fixtures que j'écris sont des fixtures auxquelles je crois déjà.** Tous les tests de
la carte mangeaient deux écrans côte à côte, en nombres ronds, coin à l'origine. Les huit
scénarios de l'oracle **sans magasin** passent maintenant par le vrai pipeline
(`lbm_layout::linux::populate`, l'appel de l'agent) et l'extent est comparé à ce que le C#
a enregistré, à 1e-9 près — −682 mm de coin, une télé de 1110 mm à côté d'une dalle de
300 mm, des hauteurs fractionnaires, neuf écrans.

## Le nom d'un écran : correction d'une règle que j'avais inventée

Le spike des tailles proportionnelles (#682) posait « le nom prend 14 % de la hauteur
allumée, comme le cadre Avalonia ». **C'était faux, et la seconde moitié de la phrase
l'était aussi** : le C# ne mesure pas le nom contre la dalle mais contre **la bordure**.
`MonitorFrameView.axaml:184-186` lie la taille de police à `TopRow.Bounds.Height` par un
convertisseur `Scale` de paramètre `0.5`, et `TopRow` est la bordure étirée dans la
cellule (0, 0) de la grille — dont la hauteur est `Unrotated.TopBorder` (`:155-158`,
lignes `Auto,*,Auto`).

Deux conséquences, pas une :

- **La taille** est la moitié de la bordure du haut. Un écran à bordure fine porte un
  petit nom quelle que soit la taille de sa dalle. Sur un écran de 600×340 à bordure de
  10 mm, l'écart entre les deux règles est d'un facteur **47**.
- **La place** est la bordure du haut, sur la largeur de la dalle (cellule (0, 1)),
  alignée en bas. Le nom est imprimé **sur le plastique**, là où un vrai moniteur
  l'imprime — pas par-dessus l'image. Mon cadre le posait dans la partie allumée.

### Et ce que la vraie règle rend visible

Mesuré sur les bureaux de l'oracle, fenêtre de 1000×700 (donc boîte de 940×640) :

| bureau | bordures | police du nom |
|---|---|---|
| un seul écran | 20 mm | 13 à 17 pt |
| `grid-2x2` | 20 mm | 8,3 pt |
| `six-monitors` | 20 mm | **4,9 pt** |
| `nine-monitors-long-id` | 20 mm | 4,3 pt |
| `three-screens-mixed-scale-saved` | 6–10 mm | **1,3 à 2,1 pt** |
| `border-sections` | 3–6 mm | 1,0 à 2,0 pt |

Autrement dit, **sur tout bureau multi-écrans l'UI livrée dessine le nom entre 1 et 5
points**. En plein écran (1920×1080) les bordures de 20 mm remontent vers 8–10 pt, mais
celles de 3 à 6 mm restent sous 3 pt quoi qu'on fasse.

Le plancher de lisibilité de ce portage (7 pt, mon jugement, pas une règle du C#) n'est
donc **pas une soupape rare** : il décide du cas courant. Tel quel, la carte perd tous
ses noms dès que la fenêtre n'est pas grande. Trois issues, et c'est un choix produit :
garder le plancher et accepter une carte sans noms en petite fenêtre ; l'enlever et
dessiner des noms de 2 points comme aujourd'hui ; ou s'écarter de la règle Avalonia et
mesurer le nom autrement (une fraction du cadre, bornée par la bordure). **Rien n'est
tranché ici** : la règle portée est celle du C#, le plancher est resté à 7.

## Le cadre complet : le nom, le logo, les bandes du sondeur

Le cadre porte trois choses, et chacune a livré une surprise.

**Le nom** — voir la section précédente : moitié de la bordure du haut, sur le plastique.

**Le logo** est son symétrique : cellule (2, 1), la bordure du bas, sur la largeur de la
dalle, centré, `Stretch="Uniform"`, `Opacity="0.8"`. Trois points :

- `LogoPadding` (`MonitorFrameViewModel.cs:67-71`, un `4 × ratio` sur chaque côté) est
  **du code mort** — ces deux lignes et rien d'autre dans toute la solution. Le logo n'a
  aucun padding aujourd'hui. Je m'apprêtais à le porter depuis l'arithmétique seule.
- `Foreground="LightGray"` n'est pas repris littéralement. Avalonia peut le coder en dur
  parce que son bezel est une paire de brosses **fixes** ; le nôtre vient du thème, donc
  un gris clair fixe serait clair sur clair. On garde l'intention, pas la valeur.
- La résolution du chemin (`icon/Pnp/{code}?icon/Pnp/LBM`) est dans `lbm-icons` : le `?`
  est un repli, la casse est ignorée des deux côtés, et **il n'y a pas de dernier
  recours** — le `icons/default` du C# ne peut jamais être enregistré, le chargeur ne
  produisant que des clés préfixées `icon/`.
- Trouvaille de données : `Asus.ATK.ACI.ASU.svg` ne répondait pas à **`AUS`**, le code
  qu'ASUS écrit réellement dans son EDID. Sur `six-monitors`, un ASUS avait son logo et
  l'autre non. La table d'alias étant le nom du fichier, le correctif est le nom — et il
  vaut aussi pour l'app Avalonia livrée.
- **Piège de portage** : une clé est un nom dans l'espace de ressources, pas un chemin.
  La construire en imprimant un `Path` donne `icon\pnp\del` sous Windows et plus rien ne
  résout. Le spike y échappait parce que sa racine laissait un préfixe d'un seul segment.

**Les bandes du sondeur** montrent ce que le moteur va *faire*, par opposition à ce que le
layout dit : rouge pour un mur, vert pour un passage. Deux règles faciles à manquer — un
run **nomme son dernier pixel** (d'où le `+1`, sans lequel chaque bande est courte d'un
pixel : 224,79 sur 225), et les runs sont en **pixels du bureau**, donc un écran qui ne
commence pas à l'origine doit être décalé avant d'être mis à l'échelle. L'épaisseur de
5 points n'est **pas** mise à l'échelle : c'est une marque sur le dessin, pas une mesure du
bureau.

## Glisser un écran : la première édition

`lbm-ui::drag`, porté de `FrameMover.cs`. Trois morceaux séparés parce qu'ils répondent à
des questions différentes : l'**aimantation** (millimètres en entrée, millimètres en
sortie), les **repères** qui disent pourquoi elle a accroché, et le **dépôt**, seul à
écrire quelque chose. La fenêtre garde désormais son `Layout` au lieu de le jeter après la
détection — en mémoire seulement, rien n'est sauvé.

Cinq ancres par axe et par écran : bord du plastique, bord de la dalle, **milieu de la
dalle** (`s.X + s.Width / 2`, donc de l'image et non de l'objet), puis la paire opposée.
La plus petite correction par axe l'emporte, dans la limite de **10 mm** (`:164`, comparé
avec `>`, donc 10 accroche encore). Les deux axes se décident séparément.

Quatre faits du C# qu'on ne devine pas en lisant le code :

- **Les ancres de natures différentes s'apparient.** `:187-190` croise les cinq contre les
  cinq, donc un bord de plastique atterrit sur la dalle du voisin. C'est ce qui permet
  d'aligner un écran par son cadre contre l'image d'un autre — et ça veut dire qu'un écran
  décalé de moins qu'une épaisseur de cadre a **plusieurs règles en portée à la fois**.
- **La carte ne se remet pas à l'échelle pendant le glissement.** `GetRatio` lit
  `PhysicalBounds`, que le layout ne republie qu'à l'écriture d'une position — ce qui
  n'arrive qu'au dépôt. Un écran traîné hors du bureau sort donc de la carte au lieu de la
  rétrécir sous le pointeur.
- **Glisser le primaire le long d'une rangée jointive ne fait rien du tout.** Le primaire
  est l'origine de l'espace en millimètres : on décale donc tous les *autres* en sens
  inverse (`:123-136`), ils chevauchent, et la compaction qui suit (`:131-144`) les remet
  exactement où ils étaient. Trouvé en écrivant le test du primaire avec un glissement
  diagonal et en regardant la moitié horizontale s'évaporer.
- **La compaction peut déplacer des écrans que l'utilisateur n'a pas touchés**, d'où la
  relecture du layout entier après chaque dépôt plutôt qu'une mise à jour de l'écran
  déposé.

Deux écarts assumés, tous deux annotés dans le code :

- **La sélection** passe par `Sense::click_and_drag`, qui laisse egui décider à la distance
  et à la durée, au lieu de l'égalité exacte appui/relâchement de `:113`.
- **L'épaisseur des repères est choisie, pas portée** : `:257` calcule `5 : 2` et ne le lit
  jamais, les deux `StrokeThickness` étant commentés en dessous. Il est donc possible que
  ces traits ne soient visibles dans **aucune version livrée** de l'app Avalonia — non
  vérifié, faute de sources Avalonia sous la main. Valeurs retenues : 2 et 1, **regardées
  à l'écran et validées par le mainteneur le 2026-09-14**.

Et un manque **à moi**, rattrapé en construisant le binaire à donner à essayer, pas par un
test : **Ctrl** pendant le glissement désactive l'aimantation (`MonitorLocationView.axaml.cs:184-185`
→ `FrameMover.cs:183`). Sous 10 mm tout glissement est happé, donc un bureau dont les
écrans sont réellement à quelques millimètres l'un de l'autre était **indescriptible**.
Mes tests vérifiaient que l'aimantation marche ; aucun ne vérifiait qu'on peut y échapper.

## Deux choses que l'item 2 a révélées sur le reste

- **Le rapport de sondage arrive en XML dans une chaîne JSON** (`api.rs`,
  `"Payload": "<ProbeReport/>"`), forme héritée du frontend C#, qui le lit encore jusqu'à
  la phase 7. Quelqu'un devra le parser côté Rust — ou l'agent gagnera une forme JSON à
  côté du XML. Question de contrat filaire, pas de dessin, **non tranchée**.
- **Le menu contextuel n'est pas un travail de vue.** Ses deux entrées sont des commandes
  du présentateur, pas de l'écran, et la seconde — « Apply layout to system config » —
  **écrit la topologie d'écrans du système**. Le C# le fait par `IDisplayController`
  (`LinuxDisplayController.cs:85` avec kscreen-doctor, ses relances quand le compositeur
  écrase les positions et sa vérification de recouvrement ; `WindowsDisplayController.cs:57`).
  Rien de tout cela n'existe en Rust. Ce n'est pas un trou du plan : la ligne 79 confie la
  topologie au **frontend** et non à l'agent — c'est donc une capacité à écrire avant que
  le menu ait un sens.

## Ce qui a été tranché

### 1. « Non enregistré » est une **référence**, pas un drapeau

La formule du plan, « DTO courant ≠ DTO stocké », a un piège. Elle vaut pour un document écrit
par la version courante ; pour un document **antérieur** le DTO diffère sans aucune édition,
parce qu'enregistrer le **migrerait**. Ce n'est pas une conjecture :
`save_after_load_v56_reproduces_the_stored_files` (lbm-store) charge la fixture `v5.6-current`,
l'enregistre sans y toucher, et compare à un golden **différent** — `v5.6-current-saved`, où la
priorité de la mise en page a remonté dans `options.json` et où chaque `+` du raccourci de
secours est une échappée unicode. Une fenêtre qui comparerait au magasin s'ouvrirait en proposant
d'enregistrer une mise en page que personne n'a touchée, sur tout profil d'une version
antérieure.

La comparaison n'est donc pas contre le magasin. Une **référence** est prise au chargement et à
chaque enregistrement qui aboutit ; « non enregistré » devient *document courant ≠ référence*
(`lbm_app::saved::Reference`). Trois conséquences, et chacune corrige quelque chose que le
drapeau porté rate :

- une **migration n'est pas une édition** : la référence est prise *après* le chargement ;
- **une édition défaite à la main redevient enregistrée.** Le `Saved` du C#
  (`MonitorsLayout.Saved`) est un loquet que seuls un chargement ou un enregistrement
  rabaissent : glisser un écran et le remettre exactement laissait Enregistrer allumé ;
- **un enregistrement qui aboutit éteint les boutons.** La fenêtre envoyait `SaveLayout` et
  n'entendait jamais la réponse d'une façon qui touche le drapeau : **Enregistrer restait allumé
  sur une mise en page déjà sur le disque.** Bug livré, trouvé en câblant la décision.

Ce qu'une référence prise à la réponse aurait raté : la fenêtre reste utilisable pendant qu'un
enregistrement est en vol, donc c'est le document **tel qu'envoyé** qui est mérité, apparié par
id — un refus ne mérite rien, une réponse à autre chose ne règle rien. La règle vit dans
`Conversation` (`earning`/`earned`) et non dans `main.rs` : le routage des messages d'un binaire
est l'angle mort où les deux bugs de #716 se cachaient.

### 2. Les captures de référence — le blocage n'était pas lavapipe

La note disait : « il faut un rastériseur logiciel (lavapipe) dans l'image du workflow, donc ça
n'a pas été tenté ». lavapipe est la partie facile — une ligne d'`apt-get`, et il rend des images
que le seuil accepte face à celles d'un vrai GPU. Le vrai blocage est le **graphe de
dépendances**, et il est côté Windows :

> `egui_kittest` rend par wgpu ; `lbm-hook` épingle `windows` 0.62 ; cargo unifie `windows-core`
> à 0.62 pour `wgpu-hal` ; `gpu-allocator` 0.28, que le back-end DX12 de `wgpu-hal` appelle, a été
> bâti contre 0.58. Leurs `ID3D12Device` sont alors deux types différents et **`wgpu-hal` lui-même
> ne compile pas**.

Rien ici ne peut le corriger, c'est à l'amont. Et une dev-dependency atteint `cargo test`, que le
job Windows lance **sur tout le workspace** : wgpu est donc tenu hors de Windows par
`[target.'cfg(not(windows))'.dev-dependencies]`.

Deux choses que le premier rendu a montrées, et qu'aucun test d'arbre n'aurait pu montrer :

- **`Harness::render` peint sur rien.** Le `Ui` d'`App::ui` n'a pas de fond, la fenêtre l'enveloppe
  dans un `CentralPanel`, et une capture sans panneau rend du blanc sur transparent — composité
  ensuite sur ce que le visionneur met derrière. La première série ressemblait à des rectangles
  noirs sur papier blanc, avec un bloc sombre parasite qui n'était que de l'alpha prémultiplié.
  **Image et application doivent être la même chose, sinon la référence épingle une fiction.**
- **À 900×600, la carte à deux écrans n'a aucun nom.** Les tests d'arbre ne l'avaient jamais vu :
  ils utilisent une fenêtre de 1200×900, où les noms survivent. C'est la question du plancher de
  7 pt rendue visible — il décide bien du cas courant, et la capture est gardée telle quelle.

### 3. Le spike des mires reste repoussé — mais pas pour la raison écrite

La raison notée ici (« le spike demanderait `eframe` ») est **morte** : `eframe` est dans le
workspace depuis #694 et la fenêtre existe. La vraie raison est le séquencement, et elle est plus
forte :

- `lbm-pattern` **ne dessine rien**. C'est un visionneur : 475 lignes de plomberie Wayland qui
  décodent un PNG dans un `wl_shm` et le plein-écranisent sur une sortie nommée, avec un
  `wp_viewport` qui garantit le 1:1 (`main.rs:216-222`). Pas de `lib.rs`, aucune API publique,
  **aucun test**, et aucun crate du workspace n'en dépend ;
- **la fonctionnalité est entièrement en C#** : `HLab.Sys.Windows.MonitorVcp.Avalonia/TestPattern.cs`
  (les sept motifs, le damier de gamma dont `ChessCell` est *la mesure*) et
  `DrawingContextExtension.cs` ; `TestPatternButtonViewModel.cs:117-127` rend le motif Avalonia
  hors écran en PNG à la résolution native, puis lance le binaire Rust ;
- son **seul consommateur** est le plugin VCP en C# (`Patterns/WaylandPattern.cs:17,36,70`).

Porter les mires, c'est donc porter VCP — item 8 de l'ordre des écrans. Rien à faire avant, et
`lbm-pattern` ne peut pas disparaître entre-temps.

### 4. Les modes deviennent une énumération fermée

La note disait que fermer la liste « revient à décider quels plugins survivent au portage ». Le
relevé la désamorce : il y a exactement **sept** enregistrements de `ViewMode` — `about`, `info`,
`location`, `resistance`, `size`, `vcp`, `wallpaper` — et **tous les sept sont dans ce dépôt**,
compilés dans l'app, découverts en balayant ses propres DLL (`Program.cs:249-264`). `ViewMode`
(`IView.cs:28`) est une classe abstraite **vide** et chaque mode une sous-classe vide : le type
*est* l'identité. Aucun plugin tiers, aucun point d'extension publié. C'est une indirection
interne sur une liste que ce dépôt possède, et la fermer ne décide rien sur le produit.

Ce qui reste une vraie question — lesquels sont portés, et quand — passe par `mode::Offered` :
un mode que le portage n'a pas atteint est **affiché et désactivé, en disant pourquoi**. Une barre
de ce qui manque vaut mieux qu'une barre qui prétend que ces fonctions n'ont jamais existé, et
elle donne une carte visible du travail restant (cinq boutons grisés sur sept aujourd'hui).

Deux comportements du C# repris, aucun des deux déclaré nulle part : presser le mode déjà allumé
revient au défaut (`MainPluginsViewModelExtension.cs:22-26` — c'est ce qui fait qu'une rangée de
bascules se comporte en groupe radio, et la seule façon de revenir au défaut, qui n'a pas de
bouton) ; et un mode qui cesse d'être disponible retombe sur le défaut (`:49-50`, le cas de VCP,
dont le bouton suit un réglage que l'utilisateur peut éteindre en regardant la vue VCP).

Un comportement **non** repris : l'ordre, qui est un tri alphabétique sur l'`Id` interne du plugin
(`MainViewModel.cs:63`), donc renommer un id réordonne la barre en silence. `Mode::ALL` garde
l'ordre livré, mais comme décision.

`Location` et `About` ont un contenu dès maintenant. Les lignes sont **non mises à l'échelle**,
contrairement au nom : en C# le contenu d'un mode est un contrôle ordinaire dans la cellule du
milieu, à la taille de police de l'app. Un écran dessiné petit reçoit **moins de lignes**, pas des
lignes plus petites.

## Ce qui attend encore une décision

**Le plancher de lisibilité du nom (7 pt).** Ce n'est pas une des quatre : c'est la question
laissée ouverte plus haut, et les captures la rendent regardable. Trois issues, et c'est un choix
produit : garder le plancher et accepter une carte sans noms en petite fenêtre ; l'enlever et
dessiner des noms de 2 points comme l'app livrée ; ou s'écarter de la règle Avalonia et mesurer le
nom autrement (une fraction du cadre, bornée par la bordure).

## Ce qui n'est pas fait, et qui n'attend personne

Le spike **Windows** des surfaces superposées (réservé à une session Windows), et tout ce qui
suit dans l'ordre des écrans du plan.
