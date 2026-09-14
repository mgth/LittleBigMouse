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

## Ce qui attend une décision

1. **« Non enregistré » : la formule du plan a un piège.** « DTO courant ≠ DTO stocké » vaut pour
   un document écrit par la version courante ; pour un document **antérieur**, le DTO diffère sans
   aucune édition, parce qu'enregistrer le **migrerait** (la priorité qui remonte dans
   `options.json`, le `+` du raccourci échappé). Mesuré : la fixture `v5.6-current` donne
   `dto_equal=false` sur un chargement neuf, `v5.6-current-saved` donne `true`.
   *Proposition* : une **empreinte de référence** prise au chargement et à l'enregistrement —
   « non enregistré » devient *DTO courant ≠ référence*. La migration ne compte alors pas comme
   une édition, une édition annulée redevient « enregistré » (ce que le drapeau actuel ne sait pas
   faire), et rien n'est relu dans le magasin.
2. **Captures de référence en CI** : `egui_kittest` sait rendre avec wgpu, mais il faut un
   rastériseur logiciel (**lavapipe**) dans l'image du workflow. C'est le seul point qui touche la
   CI, donc il n'a pas été tenté.
3. **Spike des mires** (`with_monitor`) : repoussé. `lbm-pattern` est lancé comme aide par le
   plugin **VCP en C#**, donc il ne peut pas disparaître avant le portage de VCP — item 8 de
   l'ordre des écrans. Le spike demanderait `eframe` (winit + glow) pour une réponse sans
   consommateur avant longtemps.
4. **La barre des modes** : en C# les modes sont un mécanisme de **plugins** (chaque plugin
   contribue sa `ViewMode`). Les remplacer par une énumération fermée, comme le plan le demande,
   revient à décider quels plugins survivent au portage — décision produit, pas portage mécanique.

## Ce qui n'est pas fait, et qui n'attend personne

Le spike **Windows** des surfaces superposées (réservé à une session Windows), et tout ce qui
suit dans l'ordre des écrans du plan.
