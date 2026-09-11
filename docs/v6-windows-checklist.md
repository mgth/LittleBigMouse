# Checklist Windows de la phase 3 (v6)

La sortie de la phase 3 demande, en plus d'une semaine de configuration quotidienne sous
Linux, une **checklist Windows passée sur machine réelle** : veille/réveil, dock/undock,
bureau sécurisé UAC, élévation, migration de la tâche d'autostart, scénario #607. Rien de
tout cela ne peut être vérifié par la CI ni sur la machine de développement Linux ; ce
document est ce qu'il reste à faire tourner à la main, dans l'ordre.

Le plan complet est `docs/v6-architecture-plan.md` (branche `v6`) ; ce que fait l'agent est
décrit dans `docs/v6-agent.md`.

## 0. Avant de commencer

- Construire l'agent : `cargo build --release -p lbm-agent` (dans `rust/`), binaire dans
  `rust/target/release/lbm-agent.exe`.
- Le hook reste celui d'aujourd'hui (`LittleBigMouse.Hook.exe`), posé **à côté** de l'agent
  ou nommé par `--hook` : l'agent le lance quand personne ne répond au tube.
- **Ne pas laisser tourner l'UI C# pendant les essais** : elle est encore résidente et
  pilote le hook elle-même — deux chefs pour un hook. La quitter par son icône avant.
- Deux machines sont demandées par le plan : une avec station d'accueil, une NVIDIA avec un
  écran en portrait.
- Un essai sans rien toucher de la configuration réelle :
  `lbm-agent.exe --fake-hook --config-dir C:\Temp\lbm\config --data-dir C:\Temp\lbm\data`
  (aucune souris capturée, aucun profil ni tâche planifiée de l'utilisateur touchés).

## 1. Écrans : le dump C# et le dump Rust (phase 2)

Sur chaque machine :

1. `lbm-agent.exe --dump-displays > rust.json`
2. Le jumeau C# (`DisplayDumpTests.DumpDisplays`, qui n'écrit que si la variable est
   posée) :
   ```
   set LBM_DUMP_DISPLAYS=%CD%\csharp.json
   dotnet test LittleBigMouse.Core\LittleBigMouse.DisplayLayout.Tests --filter DumpDisplays
   ```
3. Comparer : `fc rust.json csharp.json` (ou un diff JSON). **Attendu : aucune différence**
   — ids de moniteurs, id de mise en page, tailles physiques, DPI, positions.

C'est ce qui garantit que les profils enregistrés par la 5.x seront retrouvés par l'agent.

## 2. Import du registre (D2)

Sur une machine qui a servi avec la 5.x :

1. Vérifier que `%LOCALAPPDATA%\Mgth\LittleBigMouse\options.json` **n'existe pas** encore.
2. Lancer l'agent une fois, puis le quitter.
3. Attendu : `options.json`, `models.json` et `layouts\*.json` sont apparus ; leurs valeurs
   sont celles de `HKCU\SOFTWARE\Mgth\LittleBigMouse` (comparer quelques valeurs :
   `Enabled`, la position d'un moniteur, une résistance de bord).
4. Le registre est **inchangé** (la 5.x doit continuer de fonctionner).
5. Relancer l'agent : le journal ne parle plus d'import (une seule fois).
6. Une mise en page dont le nom de clé est un condensé (#589, plus de 255 caractères) est
   signalée comme non importée si son id n'a pas pu être reconstruit — vérifier le journal.

## 3. Tubes et API

1. L'agent trouve le hook : au démarrage, `agent.log` dit `hook endpoint:
   \\.\pipe\LittleBigMouse-v1-session-<n>` puis `hook connected`.
2. L'API : `frontends: \\.\pipe\LittleBigMouse-Agent-v1-session-<n>`.
3. Un client non élevé doit pouvoir écrire dans le tube d'un agent **élevé** (étiquette
   d'intégrité) : lancer l'agent en administrateur, puis un client simple (script PowerShell
   ou Python ouvrant le tube, `Hello`) — la réponse doit arriver.
4. Un client d'une **autre session** (changement rapide d'utilisateur) ne doit pas être
   servi.

## 4. Tray

1. L'icône apparaît dans la zone de notification, sa bulle donne l'état (Running, Stopped,
   Paused, écran éteint).
2. Le menu : Ouvrir, Start, Stop, Rafraîchir, Quitter. Start et Stop changent l'icône et
   l'état du hook ; Quitter fait partir le hook **puis** l'agent.
3. Ouvrir ne fait rien tant que `--ui` ne nomme pas un frontend (voulu en phase 3).
4. Redémarrer l'explorateur (`taskkill /f /im explorer.exe`, puis le relancer) : **l'icône
   revient** (`TaskbarCreated`).

## 5. Démarrage avec la session (tâche planifiée, D6)

1. Sur une machine avec une tâche 5.x : noter son nom
   (`schtasks /Query /TN LittleBigMouse_<domaine>_<utilisateur> /XML`) et ce qu'elle lance.
2. **Attention** : un agent de développement réécrit cette tâche sur lui-même dès le premier
   enregistrement (comme le faisait n'importe quel build C#). Sur une machine qui doit
   continuer à démarrer la 5.x, faire l'essai avec `--config-dir` (aucune tâche touchée) ou
   remettre la tâche ensuite.
3. Après un Start/Stop de l'agent : la tâche lance `lbm-agent.exe`, déclencheur à l'ouverture
   de session, relance sur échec, sans limite de durée.
4. Fermer la session et la rouvrir : l'agent démarre seul, le hook suit.
5. Avec `StartElevated` : la tâche est enregistrée en `HighestAvailable` ; sans droits, elle
   retombe en tâche simple (et non plus rien).

## 6. Veille et réveil

1. Mettre la machine en veille pendant que le moteur tourne, réveiller.
2. Attendu : le hook a décroché à l'extinction de l'écran (`Suspended`), le moteur revient
   après le réveil sans intervention (stabilisation puis chien de garde), l'icône repasse au
   vert.
3. Même essai avec la seule extinction de l'écran (veille de l'écran, pas de la machine).

## 7. Dock, undock, et le scénario #607

1. Moteur en marche, retirer la station d'accueil : la mise en page est reconstruite, le
   curseur **n'est pas piégé** dans une zone fantôme (#607) ; il atteint tous les bords.
2. Remettre la station : la mise en page revient, le moteur aussi.
3. Brancher et débrancher un écran à chaud, changer la résolution et l'échelle d'un écran.

## 8. Bureau sécurisé (UAC) et sessions

1. Moteur en marche, déclencher une invite UAC (lancer quelque chose en administrateur).
2. Pendant l'invite, la souris doit rester utilisable ; après, le moteur doit être de nouveau
   accroché (l'agent voit le retour de session et refait le tour des écrans).
3. Verrouiller (Win+L) puis déverrouiller : même attente.
4. Changement rapide d'utilisateur : chaque session a son agent, son hook et ses tubes ;
   l'agent d'une session ne touche pas au hook de l'autre.
5. Session RDP : l'agent démarre, ne casse rien (la tâche planifiée ne démarre pas en session
   distante).

## 9. Journaux

- `%LOCALAPPDATA%\Mgth\LittleBigMouse\agent.log` : l'en-tête donne version, pid et date ;
  cinq générations sont gardées (`agent.prev.log`, `agent.prev.2.log` … `agent.prev.5.log`).
- `hook.log` : ce que dit le hook lancé par l'agent.
- Une erreur inattendue dans l'un ou l'autre est un résultat de checklist à rapporter.

## Ce qui n'est pas encore là (phase 3 en cours)

- L'exclusion par focus est toujours dans le hook (D4 la déplacera dans l'agent).
- Le fond d'écran « span » n'est pas ré-appliqué après reconstruction.
- L'UI C# est encore résidente : elle devient un frontend de l'agent en phase 4, et c'est
  elle qui ouvrira la fenêtre depuis le tray (`--ui`).
