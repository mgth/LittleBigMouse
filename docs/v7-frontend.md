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
