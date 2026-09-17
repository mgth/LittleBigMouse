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

## Suite, dans l'ordre

1. Donner un parcours egui aux commandes primaire / attachement / détachement déjà
   présentes dans la couche plateforme ; vérifier les changements réels de topologie.
2. Porter Size et Info, puis l'éditeur de sections de résistance. Examiner #523 pour
   placer les champs dans une zone de propriétés lisible à toute échelle.
3. Intégrer les règles et bandes sur les vrais écrans, puis valider le backend Windows
   et les DPI mixtes. Le crate overlay reste actuellement un prototype Linux.
4. Porter l'éditeur de fond d'écran et `IDesktopWallpaper`, puis qualifier le premier
   périmètre egui sur matériel (veille, dock, UAC, reprise, démarrage et mémoire).
5. VCP/mires, calibration, télécommandes, import/export et mise à jour ; bascule du
   packaging et retrait de .NET une fois les fonctions conservées remplacées.

Essais manuels encore nécessaires : lancement installé sans agent, agent élevé sous
Windows, débranchement avec édition en cours, changement de résolution/DPI, fermeture
pendant un aperçu et ouverture depuis le tray. Ne pas lancer l'agent de développement
sur les données réelles uniquement pour valider le transport.
