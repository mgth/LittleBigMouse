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

## Suite, dans l'ordre

1. Raccorder `Probe` aux bandes de la carte ; fixer sa représentation Rust tout en
   conservant le contrat utilisé par Avalonia.
2. Donner un parcours egui aux commandes primaire / attachement / détachement déjà
   présentes dans la couche plateforme ; vérifier les changements réels de topologie.
3. Porter Size et Info, puis l'éditeur de sections de résistance. Examiner #523 pour
   placer les champs dans une zone de propriétés lisible à toute échelle.
4. Intégrer les règles et bandes sur les vrais écrans, puis valider le backend Windows
   et les DPI mixtes. Le crate overlay reste actuellement un prototype Linux.
5. Porter l'éditeur de fond d'écran et `IDesktopWallpaper`, puis qualifier le premier
   périmètre egui sur matériel (veille, dock, UAC, reprise, démarrage et mémoire).
6. VCP/mires, calibration, télécommandes, import/export et mise à jour ; bascule du
   packaging et retrait de .NET une fois les fonctions conservées remplacées.

Essais manuels encore nécessaires : lancement installé sans agent, agent élevé sous
Windows, débranchement avec édition en cours, changement de résolution/DPI, fermeture
pendant un aperçu et ouverture depuis le tray. Ne pas lancer l'agent de développement
sur les données réelles uniquement pour valider le transport.
