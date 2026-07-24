[PRD]
# PRD: Installateur Arthur Workflow

## Changelog

| Version | Date | Author | Summary |
|---------|------|--------|---------|
| 1.1 | 2026-07-22 | Arthur Jean | Migration de la stack vers un binaire Rust natif avec Ratatui |
| 1.0 | 2026-07-22 | Arthur Jean | Définition initiale du CLI global d'installation des skills et agents Arthur Workflow |

## Problem Statement

1. Le dépôt publie le workflow réel d'Arthur, soit 50 skills, 3 agents Claude Code et 3 agents Codex, mais le parcours documenté avec le CLI Vercel Skills n'installe que les skills. Les agents restent une opération manuelle et provider-specific.
2. Une installation manuelle répartie entre `$HOME/.agents`, `$HOME/.claude` et `$HOME/.codex` ne dispose ni d'un ownership explicite, ni d'un plan avant mutation, ni d'un rollback. Une collision peut donc écraser un contenu utilisateur ou produire un état partiel.
3. Plusieurs sources contiennent encore des chemins propres à la machine d'Arthur. Les distribuer tels quels empêcherait un utilisateur tiers d'obtenir un workflow fonctionnel, même si les noms, prompts, modèles et comportements métier sont corrects.
4. Il n'existe pas de contrat automatisable pour inspecter, adopter, mettre à jour, diagnostiquer ou désinstaller l'installation complète. La parité avec le dépôt ne peut pas être prouvée de manière reproductible.

**Why now:** le dépôt possède désormais une séparation explicite entre agents Claude Code et agents Codex et reflète le workflow complet d'Arthur. Le prochain levier n'est plus d'ajouter des assets, mais de rendre leur reproduction publique déterministe, portable et réversible.

## Overview

Le produit est un CLI Rust 2024 distribué comme binaire natif autonome, avec une interface Ratatui lorsque le terminal est interactif, un renderer ligne à ligne `--plain` et un contrat JSON déterministe pour l'automatisation. Son exécutable est `arthur-skills`; le workspace Cargo vit à la racine et le crate binaire dans `crates/arthur-skills`. La toolchain de référence est Rust 1.95.0, épinglée par `rust-toolchain.toml`.

Le binaire embarque une version immuable du catalogue et un manifeste SHA-256 généré avant compilation. L'installation copie les skills dans la source canonique `$HOME/.agents/skills`. Claude Code reçoit une activation par skill dans `$HOME/.claude/skills`: symlink relatif sous Unix, copie gérée sous Windows pour fonctionner sans Developer Mode ni privilèges administrateur. Codex lit directement la source canonique. Cette découverte Codex est intrinsèque: tant que le catalogue canonique existe, choisir ou retirer l'intégration Codex gouverne ses agents, mais ne constitue pas une frontière de visibilité des skills. L'UI doit l'expliquer avant confirmation. Les agents provider-specific sont copiés dans `$HOME/.claude/agents` et `${CODEX_HOME:-$HOME/.codex}/agents`. Les evals de développement ne sont jamais embarquées ni installées.

Chaque commande passe par le même moteur: découverte de l'état réel, calcul d'un plan typé, confirmation éventuelle, staging, application transactionnelle, écriture d'un receipt versionné et rollback en cas d'échec. Le CLI ne possède que les chemins enregistrés dans ce receipt. Il ne remplace ni ne supprime un asset étranger. `adopt` permet de transférer explicitement une installation existante après preuve par hash et cible de symlink.

## Goals

| Goal | Month-1 Target | Month-6 Target |
|------|---------------|----------------|
| Reproduire le catalogue complet | 100% des assets attendus installés dans chaque matrice provider supportée | 100% à chaque release du catalogue |
| Réduire l'onboarding | Une invocation `arthur-skills` après acquisition du binaire et moins de 3 minutes jusqu'au diagnostic vert | Deux commandes shell maximum depuis une machine vierge, acquisition incluse, au P95 |
| Garantir la sûreté des mutations | 0 écrasement ou suppression d'asset non possédé dans la suite de fault injection | 0 régression sur toutes les releases |
| Garantir la récupération | 100% des points de panne simulés restaurent l'état initial ou produisent `RECOVERY_REQUIRED` avec tous les backups | 0 état partiel silencieux sur Linux, macOS et Windows supportés |
| Rendre l'état explicable | `status --json` et `doctor --json` couvrent 100% des assets gérés | Schéma rétrocompatible sur toute la major v1 |

## Target Users

### Adopteur du workflow Arthur

- **Role:** développeur ou créateur technique qui veut reprendre le workflow public d'Arthur sans le reconstruire asset par asset.
- **Behaviors:** utilise Codex, Claude Code ou les deux, travaille en terminal et accepte une distribution opinionated.
- **Pain points:** ne sait pas quels fichiers copier, où activer les skills, comment installer les agents associés, ni comment éviter les collisions avec sa configuration existante.
- **Current workaround:** lance plusieurs commandes Vercel Skills, puis copie manuellement les agents et corrige les chemins locaux.
- **Success looks like:** sélectionne ses providers, valide un plan groupé par opération et destination, obtient le catalogue complet et un `doctor` vert sans perdre ses propres assets.

### Utilisateur existant et multi-machine

- **Role:** Arthur ou un utilisateur ayant déjà installé une version du catalogue via Vercel Skills ou manuellement.
- **Behaviors:** synchronise régulièrement le dépôt, conserve des skills personnels à côté du catalogue et attend des mises à jour reproductibles.
- **Pain points:** risque de double ownership avec `.skill-lock.json`, dérive silencieuse entre machines, désinstallation dangereuse et manque de preuve sur la version réellement active.
- **Current workaround:** inspecte les symlinks et compare les dossiers à la main.
- **Success looks like:** adopte l'existant sans réécriture inutile, met à jour uniquement les assets possédés et peut retirer un provider sans toucher aux autres.

## Research Findings

Key findings that informed this PRD:

### Competitive Context

- [Vercel Skills](https://github.com/vercel-labs/skills) fournit un modèle éprouvé de source canonique plus symlinks par agent, ainsi que des commandes d'ajout, liste, mise à jour et suppression. Il couvre les skills, mais pas le catalogue d'agents provider-specific de ce dépôt.
- [Claude Code](https://code.claude.com/docs/en/plugins) distingue les configurations standalone des plugins distribués. Les plugins namespacent les skills, alors que les dossiers personnels `~/.claude/skills` et `~/.claude/agents` conservent les noms nus attendus. Les [subagents Claude Code](https://code.claude.com/docs/en/sub-agents) confirment le répertoire utilisateur `~/.claude/agents`.
- [Codex](https://learn.chatgpt.com/docs/build-skills) découvre les skills utilisateur dans `$HOME/.agents/skills`; ses [subagents personnalisés](https://learn.chatgpt.com/docs/agent-configuration/subagents) résident dans `${CODEX_HOME:-$HOME/.codex}/agents`.
- Les plugins natifs Claude et Codex apportent un bon cycle de distribution, mais ne donnent pas simultanément les noms nus Claude, le stockage canonique Codex et l'installation des deux formats d'agents.
- **Market gap:** aucune option étudiée ne reproduit en une transaction le workflow exact du dépôt, ses skills partagés, ses agents provider-specific, son ownership et son cycle de vie.

### Best Practices Applied

- Utiliser une source canonique avec une activation Claude possédée par skill évite la divergence tout en préservant les assets propres à chaque provider.
- Emballer le catalogue avec le CLI et vérifier son manifeste avant toute mutation évite l'exécution ou le téléchargement opportuniste de contenu pendant l'installation.
- [Ratatui 0.30.2](https://docs.rs/ratatui/0.30.2/ratatui/fn.init_with_options.html) fournit `Viewport::Inline`, la restauration du terminal et l'adaptation au resize sans imposer l'alternate screen. Son [TestBackend](https://docs.rs/ratatui/0.30.2/ratatui/backend/struct.TestBackend.html) permet des assertions déterministes sur les buffers rendus.
- [Clap](https://github.com/clap-rs/clap) fournit sous-commandes et arguments typés, relations entre flags, aide/version générées et validation de la définition par `CommandFactory::debug_assert()`.
- [cargo-dist](https://github.com/axodotdev/cargo-dist) construit archives et installateurs natifs et génère le workflow GitHub Releases à partir du workspace Cargo et de `rust-toolchain.toml`.
- Les commandes automatisées doivent éviter toute initialisation Ratatui, ne jamais demander d'entrée et produire un schéma JSON ainsi que des exit codes documentés.
- L'installation doit séparer une phase pure de planification d'une phase impérative transactionnelle afin que `plan`, `--dry-run`, l'UI et l'exécuteur partagent exactement la même décision.

*Full research sources are linked above and recorded in the local exploration that produced this PRD.*

## Assumptions & Constraints

### Assumptions (to validate)

- **HIGH, US-001:** Ratatui 0.30.2 avec son backend Crossterm fonctionne en viewport inline sur les terminaux Linux, macOS et Windows ciblés, restaure toujours le terminal et partage la même state machine avec le renderer `--plain`.
- **MEDIUM, US-020:** cargo-dist produit de manière reproductible les cinq cibles natives retenues depuis Rust 1.95.0 et conserve les permissions applicables des assets générés.
- **MEDIUM, US-020:** les binaires macOS construits avec `MACOSX_DEPLOYMENT_TARGET=13.0` fonctionnent sur macOS 13 et supérieur sans dépendance dynamique hors bibliothèques système.
- **MEDIUM, US-003:** remplacer les chemins personnels par `$HOME`, `CODEX_HOME`, des chemins relatifs au skill ou une commande résolue via `PATH` conserve la sémantique du workflow.
- **MEDIUM, US-009:** une installation Vercel Skills existante peut être adoptée sans ambiguïté à partir de `.skill-lock.json`, des hashes réels et des cibles de symlink.
- **MEDIUM, US-010:** les symlinks relatifs sont stables sur les filesystems Unix ciblés et les copies gérées Windows conservent contenu, ownership et rollback.

### Hard Constraints

- Le catalogue installé doit conserver les noms de skills, noms d'agents, prompts, permissions et modèles publiés. Aucun modèle ne peut être substitué silencieusement.
- La source canonique des skills est `$HOME/.agents/skills`; Codex ne reçoit pas un second exemplaire ni un symlink provider inutile.
- Claude Code reçoit une activation possédée par skill: symlink relatif sous Unix, copie gérée sous Windows. Le dossier complet `skills` du provider ne doit jamais être remplacé par un symlink ou une copie globale.
- Le dernier exemplaire canonique possédé ne peut être supprimé que lorsqu'aucun provider sélectionné ne le référence.
- La sélection provider gouverne les activations et agents gérés, pas l'isolation des skills: tout Codex présent sur la machine peut découvrir `$HOME/.agents/skills` tant que le catalogue canonique existe.
- Chaque référence interne nécessaire au runtime, notamment `~/.claude/skills/_shared`, doit résoudre vers un asset versionné et embarqué dans le binaire. Une référence manquante bloque la release.
- Le CLI utilise Rust 1.95.0, edition 2024, Cargo et un `Cargo.lock` versionné. Le crate racine interdit `unsafe`; les lints workspace refusent `unwrap()` et `expect()` en production, sauf dérogation locale avec `#[allow(..., reason = "invariant vérifié à la frontière")]`.
- Aucun runtime ou gestionnaire de packages JavaScript n'entre dans le build, les tests, l'installation ou l'exécution du CLI.
- V1 supporte Linux, macOS et Windows sans exiger de privilèges administrateur.
- Les artefacts v1 ciblent Linux musl x86_64/ARM64, macOS 13+ x86_64/ARM64 et Windows x86_64 MSVC. Tout chemin d'entrée non UTF-8 est refusé avant mutation avec une représentation hexadécimale lossless dans le diagnostic.
- Les tests automatisés doivent utiliser un HOME temporaire et ne jamais muter `/home/arthur/.agents`, `/home/arthur/.claude` ou `/home/arthur/.codex`.
- Le CLI ne doit pas modifier les fichiers globaux `AGENTS.md`, `CLAUDE.md`, `config.toml`, les credentials, sessions, caches ou bases gérées par les providers.
- Le catalogue complet est installé par défaut. V1 n'offre ni profil réduit ni sélection skill par skill.

## Quality Gates

These commands must pass for every user story:

- `cargo fmt --all -- --check` - vérifie le format Rust sans modifier les fichiers.
- `cargo check --workspace --all-targets --all-features` - compile toutes les surfaces du workspace.
- `cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::unwrap_used -D clippy::expect_used` - refuse tout warning ainsi que `unwrap()` et `expect()` sans dérogation locale motivée.
- `cargo test --workspace --all-targets --all-features` - exécute unités, intégrations filesystem, buffers Ratatui et scénarios process.
- `cargo llvm-cov --workspace --all-features --fail-under-regions 90` - mesure les régions LLVM du workspace et échoue sous 90%.
- `cargo deny check` - vérifie advisories, licences, sources et duplications selon `deny.toml`.

Aucun navigateur n'est requis. Pour les stories UI, la preuve visuelle repose sur `ratatui::backend::TestBackend`, des buffers textuels versionnés, le renderer `--plain` et des scénarios pseudo-terminal.

## Epics & User Stories

### EP-001: Fondations portables du catalogue

Établir un binaire Rust autonome et un catalogue immuable qui représentent fidèlement le repo sans dépendre de la machine d'Arthur.

**Definition of Done:** le binaire release peut être construit et exécuté sans runtime externe, le manifeste couvre tous les assets attendus, aucun eval n'est embarqué et aucune référence machine-bound bloquante ne subsiste.

#### US-001: Valider le socle Rust et Ratatui

**Description:** As a mainteneur, I want prouver le binaire Rust, Ratatui inline et le renderer plain avant de bâtir le CLI so that runtime, terminal et distribution reposent sur une compatibilité observée.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** None

**Acceptance Criteria:**

- [ ] Given Rust 1.95.0 épinglé, when `cargo run -p arthur-skills -- --help` et le binaire release exécutent l'aide, then ils terminent avec le code 0 et n'exigent aucun runtime dynamique de langage.
- [ ] Given un pseudo-terminal Linux ou macOS, when Ratatui utilise `Viewport::Inline`, reçoit clavier et resize puis termine normalement, en erreur ou par panic contrôlée, then raw mode, curseur et styles sont restaurés avant le retour au shell.
- [ ] Given `--plain` ou `TERM=dumb`, when le même parcours de sélection s'exécute, then aucune raw mode, cursor addressing, couleur ou alternate screen n'est activée et chaque décision reste disponible ligne par ligne.
- [ ] Given le workspace, when les dépendances sont résolues, then elles proviennent de crates.io ou du workspace versionné et aucun manifeste, runtime ou installateur JavaScript n'est requis.
- [ ] Given le scaffold terminé, when ses fichiers sont inspectés, then `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, edition 2024 et `#![forbid(unsafe_code)]` sont présents; les lints workspace refusent `clippy::unwrap_used` et `clippy::expect_used`, sauf `allow` local portant une raison qui documente l'invariant déjà validé.
- [ ] Given Ratatui inline ou la toolchain native incompatible avec une plateforme cible, when le spike échoue, then US-001 devient bloquée avec la cible et le transcript minimal, sans choisir une seconde stack silencieusement.

#### US-002: Générer le manifeste immuable du catalogue

**Description:** As a mainteneur, I want générer un manifeste versionné depuis les sources du repo so that chaque installation puisse prouver exactement quels assets elle applique.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-001

**Acceptance Criteria:**

- [ ] Given le repo courant, when `build.rs` génère le manifeste, then chaque dossier top-level valide de `skills/` apparaît une fois avec nom, chemin relatif, type, taille, mode POSIX normalisé et SHA-256 de tous ses fichiers runtime.
- [ ] Given `agents/claude` et `agents/codex`, when le manifeste est généré, then seuls les agents `.md` Claude et `.toml` Codex destinés au runtime sont associés à leur provider.
- [ ] Given les documents support Claude référencés par les skills, when le manifeste est généré, then ils sont inventoriés sous `shared/claude/skills/_shared` comme assets support et non comme skills publics.
- [ ] Given `agents/codex/evals`, when le binaire est construit, then aucun fichier de cet arbre ne figure dans le manifeste, les bytes embarqués ou les archives release.
- [ ] Given deux générations sans changement de source, when les sorties sont comparées octet par octet, then elles sont identiques et triées de manière déterministe.
- [ ] Given un doublon de nom, un chemin absolu interdit, un traversal `..`, tout symlink source même interne au repo, un mode non supporté ou un fichier requis absent, when la génération s'exécute, then la compilation échoue avant linkage avec le chemin et la règle violée; les sources runtime acceptées sont uniquement des dossiers et fichiers réguliers afin que l'embarquement conserve sans ambiguïté type, contenu et mode.

#### US-003: Rendre les assets indépendants de la machine source

**Description:** As an adopteur, I want des skills et agents résolus depuis mon environnement so that le workflow fonctionne hors de `/home/arthur` sans perdre sa sémantique.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-002

**Acceptance Criteria:**

- [ ] Given les skills CLI qui référencent `/home/arthur/.agents/skills`, when leur source est préparée pour la distribution, then ils résolvent leur propre dossier via `$HOME/.agents/skills`, une variable métier dédiée ou leur emplacement runtime documenté.
- [ ] Given les trois agents Codex et leur serveur Paneflow désactivé, when ils sont packagés, then la commande Paneflow est résolue par `PATH` et ne contient aucun chemin personnel absolu.
- [ ] Given les noms, prompts, modèles et permissions des agents, when la portabilisation est diffée, then ces champs restent sémantiquement identiques à la version source validée.
- [ ] Given chaque chaîne ressemblant à un chemin dans skills, références et agents, when la policy de portabilité s'exécute, then `/home/arthur` et les chemins machine-bound sont bloqués, tandis que `$HOME`, `~/.bun`, les racines provider documentées et les chemins relatifs internes sont classés par une allowlist contextuelle.
- [ ] Given les références `~/.claude/skills/_shared`, when la release est préparée, then les quatre documents support du workflow courant sont versionnés sous `shared/claude/skills/_shared` et chaque référence transitive résout vers le catalogue embarqué.
- [ ] Given un HOME contenant des espaces ou des caractères Unicode, when un helper construit ou exécute un chemin de skill, then le chemin est transmis sans découpage shell ni interpolation non échappée.
- [ ] Given une référence interne non packagée, une variable nécessaire absente ou une commande optionnelle introuvable, when l'asset est validé, then la release échoue pour une dépendance interne et `doctor` avertit pour une capacité externe sans réécrire l'agent.

#### US-004: Valider les contrats provider-specific

**Description:** As a mainteneur, I want valider chaque asset contre son provider cible so that une release ne publie pas silencieusement un agent mal classé ou incomplet.

**Priority:** P0
**Size:** S (2 pts)
**Dependencies:** Blocked by US-002, US-003

**Acceptance Criteria:**

- [ ] Given un agent Claude, when le catalogue est validé, then son frontmatter, son modèle, ses tools et son corps passent le validateur complet de la version Claude Code de référence, initialement 2.1.217.
- [ ] Given un agent Codex, when le catalogue est validé, then son TOML complet, dont `default_permissions`, tables `permissions`, MCP, modèle et instructions, passe le validateur de la version Codex de référence, initialement 0.144.6.
- [ ] Given les 50 skills de la release initiale, when leurs métadonnées sont validées, then les noms de dossier et noms publics sont uniques et conservés sans namespace ajouté.
- [ ] Given un modèle inconnu ou une version provider sous le minimum validé, when le catalogue est inspecté à l'installation, then l'écart rend `doctor` non sain et le modèle publié n'est ni remplacé ni supprimé.
- [ ] Given un HOME temporaire, when les CLI provider de référence chargent les agents packagés via leur surface de validation ou de démarrage sans authentification, then aucun agent n'est rejeté; si cette surface n'existe pas, une fixture versionnée du schéma complet doit couvrir le même contrat.
- [ ] Given un asset associé au mauvais provider, un format non parseable ou un champ provider non supporté, when la release est construite, then la compilation échoue avec l'asset fautif et aucun binaire n'est produit.

---

### EP-002: Planification, ownership et transactions

Construire un moteur filesystem unique qui explique chaque changement avant de l'exécuter, possède uniquement ce qu'il a créé ou adopté et restaure l'état initial sur échec.

**Definition of Done:** chaque commande mutante consomme le même plan déterministe, toutes les mutations sont couvertes par le receipt et les fault injections prouvent le rollback sans perte de contenu étranger.

#### US-005: Résoudre les racines et stratégies provider

**Description:** As a développeur du CLI, I want une registry provider typée so that les chemins et stratégies d'activation ne soient jamais dispersés dans l'UI ou les commandes.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-004

**Acceptance Criteria:**

- [ ] Given un environnement standard, when les chemins sont résolus, then les skills canoniques ciblent `$HOME/.agents/skills`, Claude cible `$HOME/.claude/{skills,agents}` et Codex cible `${CODEX_HOME:-$HOME/.codex}/agents`.
- [ ] Given la registry, when un provider est inspecté, then elle expose un identifiant stable, ses labels UI, sa stratégie skills, sa stratégie agents et ses capacités sans condition provider dans les renderers Ratatui ou plain.
- [ ] Given `CODEX_HOME` relatif, vide ou pointant vers un fichier, when Codex est sélectionné, then le plan échoue avec une erreur de configuration et ne crée aucun chemin.
- [ ] Given `HOME` absent, non absolu ou non accessible, when une commande dépend du filesystem utilisateur, then elle termine avant scan avec un code d'environnement documenté.
- [ ] Given un receipt existant, when les identités lexicales ou réelles de `HOME`, de la racine canonique ou de `CODEX_HOME` diffèrent de celles enregistrées, then toute mutation concernée est refusée et l'utilisateur doit restaurer l'environnement original avant migration.
- [ ] Given un symlink Claude planifié, when il est validé, then l'emplacement du lien reste sous la racine Claude réelle et sa cible est exactement le skill canonique possédé correspondant; tout autre escape est refusé.

#### US-006: Inspecter l'état et produire un plan déterministe

**Description:** As an utilisateur, I want voir le plan exact et ses conflits avant mutation so that je puisse comprendre et automatiser l'installation sans surprise.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-005

**Acceptance Criteria:**

- [ ] Given un HOME quelconque, when `plan` s'exécute, then chaque asset est classé `create`, `update`, `remove`, `noop`, `adoptable`, `drifted` ou `conflict` avec source, destination, owner et raison.
- [ ] Given le même catalogue, le même receipt et le même filesystem, when le plan est calculé deux fois, then l'ordre et le contenu sérialisé des opérations sont identiques.
- [ ] Given `plan` ou `--dry-run`, when l'analyse se termine, then aucun fichier, dossier, symlink, timestamp ou receipt utilisateur n'est créé ou modifié.
- [ ] Given un symlink cassé, cyclique ou dirigé vers une autre source canonique, when il est inspecté, then il est classé explicitement sans suivre une boucle ni considérer la cible comme possédée.
- [ ] Given un fichier possédé dont le mode exécutable diffère du receipt, when il est inspecté, then le plan le classe `drifted` même si son SHA-256 est inchangé.
- [ ] Given un chemin non lisible, une erreur de permission ou un type inattendu, when le scan le rencontre, then le plan devient non applicable, conserve les résultats sûrs déjà calculés et liste l'erreur exacte.

#### US-007: Construire le commit transactionnel par racine

**Description:** As a développeur du CLI, I want appliquer un plan via staging, journal et commit ordonné so that chaque mutation possède une précondition, un inverse et une borne durable observable.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-006

**Acceptance Criteria:**

- [ ] Given un plan applicable sur une ou plusieurs racines filesystem, when la préparation commence, then chaque racine utilise son propre staging sibling et ses backups dont contenu, type et mode sont vérifiés avant substitution.
- [ ] Given chaque opération typée, when elle est journalisée, then elle possède préconditions, destination, device, inverse et preuve d'ownership avant son passage à `APPLYING`.
- [ ] Given une transaction nominale, when elle progresse, then un journal `0600` enregistre et `fsync` les états `PREPARED`, `APPLYING`, `COMMITTING` et `COMMITTED`, avec le receipt commit en dernier.
- [ ] Given plusieurs racines sur des devices distincts, when le commit s'exécute, then l'ordre des racines et des opérations est déterministe et aucune primitive ne dépend d'un rename cross-device.
- [ ] Given une transaction active, when une seconde commande mutante démarre, then elle échoue en moins de 250 ms sur un verrou explicite sans attendre ni modifier le filesystem.
- [ ] Given une précondition devenue fausse entre plan et application, when l'opération la revalide, then le commit s'arrête avant cette mutation et remet le journal à la borne exploitable par le moteur de récupération.

#### US-008: Gérer rollback, signaux et récupération après crash

**Description:** As an utilisateur, I want une compensation déterministe et une commande `recover` so that une erreur, un signal ou un crash ne soit jamais interprété comme un succès.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-007

**Acceptance Criteria:**

- [ ] Given une erreur recoverable injectée après n'importe quelle primitive mutante, when le rollback compensatoire termine, then chaque chemin retrouve type, contenu, permissions normalisées et cible initiaux et aucun nouveau chemin ne subsiste.
- [ ] Given plusieurs racines filesystem partiellement appliquées, when le rollback démarre, then il exécute les inverses dans l'ordre global opposé et `fsync` chaque transition `ROLLING_BACK`.
- [ ] Given SIGINT ou SIGTERM avant le commit du receipt, when le handler se déclenche, then il pose uniquement un flag atomique sans I/O, allocation ni rollback; à la prochaine borne sûre, la boucle principale bloque toute nouvelle opération, restaure le terminal, termine la compensation et retourne 130 pour SIGINT ou 143 pour SIGTERM.
- [ ] Given un crash pré-commit, when le prochain lancement inspecte le journal, then les mutations sont bloquées jusqu'à `recover`, qui reprend le rollback depuis la dernière transition durable.
- [ ] Given un crash après commit du receipt, when `recover` s'exécute, then il finalise uniquement le cleanup des backups et ne rollback pas l'état installé.
- [ ] Given `recover` incapable de restaurer une précondition, when il termine, then l'état reste `RECOVERY_REQUIRED`, tous les backups sont conservés et aucun résultat sain n'est annoncé.
- [ ] Given un second signal pendant rollback, when il est reçu, then il est mémorisé sans interrompre la compensation avant sa prochaine borne durable.

#### US-009: Adopter une installation existante entrée par entrée

**Description:** As an utilisateur existant, I want transférer explicitement les entrées compatibles de mon installation so that le nouveau CLI gère son catalogue sans retirer à Vercel Skills ses autres assets.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-006, US-007, US-008

**Acceptance Criteria:**

- [ ] Given un `.skill-lock.json` v3 conforme aux fixtures et des assets catalogue égaux en contenu, type, mode et cible, when `adopt` est confirmé, then ces entrées deviennent gérées sans réécriture de leurs assets.
- [ ] Given des activations Claude identiques au format attendu par la plateforme, when elles sont adoptées, then leurs preuves sont enregistrées et les activations ne sont pas recréées.
- [ ] Given un lock legacy contenant des entrées hors catalogue, when l'adoption commit, then une copie exacte de l'original est archivée et un lock résiduel valide conserve ces seules entrées sous ownership Vercel Skills.
- [ ] Given l'exclusivité opérationnelle confirmée par l'utilisateur, when `rewriteLegacyLock` est prêt à commit, then identité de noeud, taille, mtime et hash sont revalidés immédiatement avant le rename; tout changement observé annule l'adoption et restaure les autres opérations.
- [ ] Given qu'un gestionnaire externe ne respecte pas le verrou Arthur, when l'adoption est documentée, then v1 expose explicitement la fenêtre TOCTOU résiduelle et ne revendique aucune exclusion atomique face à ce processus.
- [ ] Given un lock ne pouvant pas être réécrit sans perdre une entrée inconnue, when `adopt` l'analyse, then l'adoption entière échoue avant mutation avec la version ou clé non supportée.
- [ ] Given un asset catalogue dont le hash, le mode, le type ou la cible diffère, when `adopt` est demandé, then l'adoption entière est bloquée et aucun ownership partiel n'est pris.
- [ ] Given `adopt --dry-run` ou un refus de confirmation, when la commande termine, then lock legacy, assets et receipt restent strictement inchangés.

---

### EP-003: Installation et cycle de vie des providers

Installer le catalogue canonique et activer Claude Code, Codex ou les deux sans interférer avec les contenus personnels déjà présents.

**Definition of Done:** une installation fraîche, une mise à jour et chaque variante de désinstallation respectent les stratégies provider, les références et l'ownership sur les 50 skills, 6 agents et 4 assets support initiaux.

#### US-010: Installer et réconcilier la source canonique

**Description:** As an utilisateur, I want une copie canonique vérifiée des skills so that tous mes providers sélectionnés consomment exactement la même version.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-008

**Acceptance Criteria:**

- [ ] Given un HOME vierge, when l'installation est appliquée, then chaque skill du manifeste est copié sous `$HOME/.agents/skills/<name>` et son hash, son type et son mode `0644` ou `0755` correspondent au manifeste embarqué.
- [ ] Given des dossiers de skills étrangers avec d'autres noms, when le catalogue est installé, then ils restent inchangés et n'apparaissent pas comme possédés dans le receipt.
- [ ] Given un chemin canonique existant non possédé portant le même nom sans preuve dans un lock Vercel Skills v3, when son contenu est identique ou différent, then l'installation ne le revendique ni ne l'écrase et indique d'utiliser `adopt` ou de résoudre le conflit.
- [ ] Given un skill possédé et inchangé, when une version plus récente du catalogue est appliquée, then seul cet asset est remplacé transactionnellement et son nouveau hash est enregistré.
- [ ] Given un skill possédé modifié localement, when `install` ou `update` le rencontre, then il est marqué `drifted`, aucune donnée locale n'est écrasée et la transaction est bloquée; v1 n'expose aucun `--force`.
- [ ] Given des dossiers ancêtres préexistants, when les modes des fichiers catalogue sont appliqués, then aucun `chmod` n'est exécuté sur ces ancêtres.

#### US-011: Activer les providers et installer leurs agents

**Description:** As a Claude Code ou Codex user, I want activer chaque surface provider avec ses propres liens, supports et agents so that le catalogue partagé conserve ses noms et chaque format reste fidèle.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-005, US-010

**Acceptance Criteria:**

- [ ] Given les racines par défaut et Claude Code sélectionné, when l'installation termine sous Unix, then chaque `$HOME/.claude/skills/<name>` géré cible exactement `../../.agents/skills/<name>` et résout vers le skill canonique possédé.
- [ ] Given les racines par défaut et Claude Code sélectionné, when l'installation termine sous Windows, then chaque `%USERPROFILE%\.claude\skills\<name>` est une copie gérée byte-identique au skill canonique, sans symlink ni privilège administrateur.
- [ ] Given les assets support sous `shared/claude/skills/_shared`, when Claude est activé, then chaque fichier est copié et possédé individuellement sous `$HOME/.claude/skills/_shared` sans revendiquer un dossier préexistant.
- [ ] Given un dossier `_shared` étranger avec des noms distincts, when Claude est activé, then ces fichiers coexistent; un support homonyme identique est `adoptable` et un support homonyme différent bloque la transaction.
- [ ] Given `$HOME/.claude/skills` contenant des assets personnels de noms différents, when l'activation s'exécute, then le dossier racine reste un vrai dossier et ces assets restent inchangés.
- [ ] Given une activation existante identique mais non possédée et absente du lock Vercel Skills v3, when `install` l'inspecte, then elle est classée `adoptable` et n'est pas recréée automatiquement.
- [ ] Given un receipt existant, un skill du catalogue encore présent dans un lock Vercel Skills v3 et des assets homonymes strictement conformes, when `install` ou `update` réconcilie l'installation, then le lock courant est archivé sous un nom non destructif, ses entrées sont transférées, les assets canoniques manquants sont recréés et l'ownership exact est enregistré dans la même transaction.
- [ ] Given Codex sélectionné, when le plan est calculé, then aucun symlink ou copie de skills n'est prévu sous `${CODEX_HOME:-$HOME/.codex}/skills`.
- [ ] Given la source canonique valide, when une intégration est enregistrée, then le receipt distingue `managed_integration` de `implicit_skill_visibility` et avertit qu'un Codex installé peut voir les skills même si ses agents ne sont pas sélectionnés.
- [ ] Given Codex sélectionné, when l'installation termine, then chaque TOML Codex est copié dans `${CODEX_HOME:-$HOME/.codex}/agents/<name>.toml` avec contenu et mode identiques au catalogue.
- [ ] Given Claude sélectionné, when l'installation termine, then chaque Markdown Claude est copié dans `$HOME/.claude/agents/<name>.md` avec contenu et mode identiques au catalogue.
- [ ] Given des agents personnels de noms différents, when les agents catalogue sont installés, then ils restent inchangés et non possédés.
- [ ] Given un asset homonyme étranger, un agent invalide, une source canonique dérivée ou une racine provider non sûre, when le plan est calculé, then la transaction sélectionnée est bloquée sans écrasement.

#### US-012: Désinstaller par provider et libérer les références

**Description:** As an utilisateur multi-provider, I want retirer Claude, Codex ou tout le workflow so that seuls les assets devenus réellement inutilisés et possédés soient supprimés.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-011

**Acceptance Criteria:**

- [ ] Given Claude et Codex sélectionnés, when `uninstall --provider claude` commit, then seules les activations skills et agents Claude possédés sont supprimés et les skills canoniques restent référencés par Codex.
- [ ] Given des fichiers support Claude possédés, when Claude est désinstallé, then seuls ces fichiers sont retirés et `_shared` n'est supprimé que s'il a été créé par le CLI et reste vide.
- [ ] Given Claude encore sélectionné, when `uninstall --provider codex` commit, then seuls les agents et métadonnées Codex possédés sont retirés et le résumé rappelle que les skills canoniques restent découvrables par Codex.
- [ ] Given le dernier provider sélectionné, when `uninstall --all` est confirmé, then les activations et agents sont retirés, les références décrémentées, puis chaque skill canonique possédé et inchangé est supprimé à refcount zéro.
- [ ] Given un asset possédé dérivé lors de `uninstall --all`, when la transaction commit, then il est conservé avec l'état `retained_unmanaged`, l'ownership actif est libéré et un rapport de désinstallation reste consultable sans autoriser sa suppression ultérieure.
- [ ] Given des assets personnels dans les racines provider, when `uninstall --all` s'exécute, then aucun chemin absent du receipt n'est supprimé et les dossiers racines non vides sont conservés.
- [ ] Given une panne pendant la désinstallation, when le rollback termine, then providers, références canoniques et receipt retrouvent ensemble leur état antérieur.

---

### EP-004: UX interactive et contrat d'automatisation

Exposer le moteur par une interface terminal avec un budget de frame P95 inférieur à 50 ms et un protocole versionné pour scripts, CI et pipes.

**Definition of Done:** toutes les commandes fonctionnent en TTY et sans TTY, le même plan alimente les deux modes, les sorties et codes sont documentés, et chaque interruption a un résultat déterministe.

#### US-013: Exposer les commandes et le contrat JSON

**Description:** As an utilisateur avancé, I want une surface de commandes cohérente et machine-readable so that je puisse inspecter ou appliquer le workflow localement et en CI.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-006, US-008

**Acceptance Criteria:**

- [ ] Given le bin `arthur-skills`, when l'aide est affichée, then elle documente `plan`, `install`, `status`, `doctor`, `update`, `uninstall`, `adopt` et `recover`, ainsi que `--provider`, `--yes`, `--dry-run`, `--json` et `--version` lorsqu'ils s'appliquent.
- [ ] Given les arguments bruts de `std::env::args_os()`, when le pré-scan rencontre `--json` avant le premier séparateur `--`, then le mode JSON gouverne toute la réponse; `--json` placé après ce séparateur reste un argument de valeur et n'active pas ce mode.
- [ ] Given le mode JSON, when `Cli::try_parse_from` retourne une commande, une aide, une version ou une erreur, then aucun chemin automatique Clap n'imprime ou ne quitte le process et stdout contient exactement une envelope v1 avec `schema_version`, `command`, `status`, `exit_code`, `catalog_version`, `transaction_id`, `providers`, `summary`, `operations`, `diagnostics` et `data`; `command` vaut `null` tant qu'aucune sous-commande n'est résolue, `transaction_id` vaut `null` avant allocation, et aide ou version sont sérialisées dans `data`.
- [ ] Given `--json --plain` avant le séparateur, when les arguments sont validés, then la combinaison contradictoire retourne une envelope JSON d'usage avec le code 2; sans `--json`, `--help` et `--version` conservent leur sortie texte humaine.
- [ ] Given deux sorties issues du même état, when elles sont comparées hors `transaction_id`, then providers, opérations et diagnostics utilisent des `Vec` triés ou `BTreeMap`; aucun `HashMap` sérialisé n'entre dans le contrat public.
- [ ] Given `CI=true`, `stdin.is_terminal() == false` ou le stream humain non TTY, when une décision manque, then Ratatui et raw mode ne sont jamais initialisés, aucun prompt n'est ouvert et la commande échoue avec l'option exacte à fournir.
- [ ] Given `--yes` sans sélection provider explicite dans un contexte non interactif, when `install` démarre, then il refuse d'inférer un provider et termine avec le code d'usage 2.
- [ ] Given succès ou no-op, erreur d'usage, conflit, environnement invalide ou échec transactionnel, when le process termine, then il utilise respectivement les codes 0, 2, 3, 4 ou 5 documentés.
- [ ] Given une option inconnue, une combinaison contradictoire ou un provider non supporté, when les arguments sont parsés, then aucune inspection mutante ne démarre et l'aide ciblée est affichée.
- [ ] Given la documentation des commandes, when un utilisateur suit le quickstart reproductible, then il trouve l'installateur cargo-dist épinglé à une release exacte, les checksums, les huit sous-commandes, les modes TTY, plain et JSON, les chemins écrits et les règles d'ownership.

#### US-014: Guider l'installation avec Ratatui

**Description:** As a nouvel adopteur, I want sélectionner mes providers et confirmer le plan dans une interface Ratatui inline so that chaque conséquence soit visible avant l'unique confirmation.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-013

**Acceptance Criteria:**

- [ ] Given un TTY, when `install` démarre sans provider, then une multi-sélection présente Claude Code et Codex avec leur état détecté et exige au moins un choix.
- [ ] Given la sélection provider, when Codex n'est pas choisi mais le catalogue canonique sera créé pour Claude, then l'écran indique avant confirmation que tout Codex local pourra quand même découvrir ces skills.
- [ ] Given les providers choisis, when le scan termine, then l'écran de revue groupe créations, mises à jour, suppressions, adoptions, no-op et conflits par racine cible avant confirmation.
- [ ] Given le catalogue opinionated, when l'utilisateur confirme, then tous les skills sont inclus et aucune sélection skill par skill n'est proposée.
- [ ] Given un plan contenant un conflit bloquant, when la revue s'affiche, then la confirmation d'application est désactivée et une commande de résolution ou `adopt` est indiquée.
- [ ] Given une transition vers revue, résultat ou erreur, when l'étape change, then Ratatui imprime le plan final et le résumé comme blocs append-only via l'insertion de lignes afin qu'ils persistent dans le scrollback.
- [ ] Given un TTY compatible, when l'interface démarre, then Ratatui utilise exclusivement `Viewport::Inline`; seules les transitions append-only sont promises persistantes et l'alternate screen n'est jamais activé.
- [ ] Given un terminal redimensionné ou trop étroit, when Ratatui appelle `autoresize`, then aucune donnée essentielle n'est tronquée sans alternative textuelle et le layout compact reste actionnable.

#### US-015: Assurer accessibilité et interruption sûre

**Description:** As an utilisateur terminal, I want contrôler tout le parcours au clavier ou dans un renderer textuel linéaire et interrompre sans corruption so that l'interface reste exploitable avec mes capacités terminal.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-014

**Acceptance Criteria:**

- [ ] Given chaque étape Ratatui, when elle est parcourue au clavier, then Tab, flèches, Espace, Entrée, Échap et Ctrl+C ont une action documentée et aucun contrôle ne dépend de la souris.
- [ ] Given `--plain`, `ARTHUR_SKILLS_PLAIN=1` ou `TERM=dumb`, when le parcours complet est exécuté, then la même state machine présente choix numérotés, plan, confirmation, progression et résultat ligne par ligne sans raw mode, cursor addressing, retour chariot, animation, couleur ou octet ESC.
- [ ] Given stdout non TTY, `--json` ou `NO_COLOR`, when une sortie est produite, then `--json` reste strictement machine-readable, le pipe humain ne contient aucun ANSI et `NO_COLOR` supprime les couleurs sans forcer à lui seul le mode plain.
- [ ] Given Ctrl+C avant mutation, when l'utilisateur interrompt, then le CLI sort avec 130 sans créer de staging, lock persistant ou receipt.
- [ ] Given Ctrl+C ou SIGTERM pendant une transaction, when le handler pose son flag atomique, then la boucle principale l'observe à la prochaine borne sûre, restaure le terminal, compense en mode plain et sort avec 130 ou 143.
- [ ] Given une seconde interruption pendant le rollback, when elle est reçue, then elle ne court-circuite pas la restauration; le CLI note l'interruption répétée et termine uniquement à la borne sûre.

---

### EP-005: Diagnostic, mise à jour et release

Fermer le cycle de vie avec une preuve d'état, une réconciliation vers chaque catalogue publié et une release testée comme un utilisateur réel.

**Definition of Done:** le binaire final peut installer, adopter, mettre à jour, diagnostiquer et désinstaller dans la matrice supportée depuis une archive cargo-dist identique à celle publiée.

#### US-016: Diagnostiquer l'état installé

**Description:** As an utilisateur installé, I want connaître la version, les racines, la dérive et les incompatibilités so that chaque écart soit prouvé avant une action corrective.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-010, US-011, US-012, US-013

**Acceptance Criteria:**

- [ ] Given une installation gérée, when `status` s'exécute, then il rapporte version CLI, version catalogue, identités de racines, providers sélectionnés, visibilité Codex implicite et nombres d'assets sains, dérivés, manquants, conflictuels, étrangers ou retenus sans mutation.
- [ ] Given `doctor`, when le diagnostic s'exécute, then il vérifie receipt, journal, hashes, types, permissions normalisées, cibles de symlink applicables, références support, permissions des racines, versions CLI providers et commandes optionnelles activées.
- [ ] Given Claude Code sous 2.1.217, Codex sous 0.144.6 ou un modèle publié non reconnu, when `doctor` le détecte, then le diagnostic est non sain, indique la version validée et ne modifie jamais l'agent.
- [ ] Given `HOME`, la racine canonique ou `CODEX_HOME` différent du receipt, when `status` ou `doctor` s'exécute, then il rapporte `root_mismatch` et toute commande mutante associée est refusée.
- [ ] Given un receipt corrompu, futur ou en `RECOVERY_REQUIRED`, when `status` ou `doctor` l'ouvre, then il fournit les diagnostics read-only disponibles et désigne `recover` lorsque cette commande est sûre.
- [ ] Given des assets `retained_unmanaged`, when `status` s'exécute, then ils sont affichés comme non possédés et ne peuvent être ciblés par update ou uninstall.

#### US-017: Réconcilier vers le catalogue embarqué

**Description:** As an utilisateur installé, I want mettre à jour vers la snapshot portée par le CLI invoqué so that assets ajoutés, modifiés ou retirés suivent un plan unique et réversible.

**Priority:** P0
**Size:** M (3 pts)
**Dependencies:** Blocked by US-012, US-016

**Acceptance Criteria:**

- [ ] Given un CLI invoqué avec un catalogue différent, when `update` est planifié, then le diff couvre assets ajoutés, changés, retirés, activations provider et métadonnées de receipt sans accès réseau.
- [ ] Given un asset retiré du catalogue, when le plan est applicable, then ses activations désirées sont supprimées avant décrément des références et le canonique inchangé n'est supprimé qu'à refcount zéro.
- [ ] Given un asset retiré mais dérivé localement, when `update` s'exécute, then il est conservé, classé conflictuel et aucune autre mutation n'est commit sans résolution.
- [ ] Given une version de catalogue identique, when `update` s'exécute, then il retourne `noop`, code 0 et zéro mtime modifié.
- [ ] Given un catalogue antérieur à celui du receipt, when `update` est demandé, then il refuse le downgrade en v1 et indique la version courante et la version cible.
- [ ] Given un utilisateur visant une version catalogue plus récente, when il consulte l'aide update, then elle précise qu'il doit d'abord acquérir le binaire cible; v1 ne met jamais son propre exécutable à jour.
- [ ] Given un journal en `RECOVERY_REQUIRED`, when `update` est demandé, then il réalise zéro mutation et exige que `recover` termine avant un nouveau plan de mise à jour.

#### US-018: Prouver le cycle de vie filesystem de bout en bout

**Description:** As a mainteneur, I want une matrice d'intégration dans des HOME temporaires so that les invariants d'ownership et rollback soient prouvés sur les systèmes supportés.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-009, US-012, US-013, US-016, US-017

**Acceptance Criteria:**

- [ ] Given Linux, macOS et Windows en CI, when la matrice fresh install s'exécute pour Claude seul, Codex seul et les deux, then chaque scénario termine avec `doctor` sain et les comptes du manifeste exacts.
- [ ] Given une seconde installation du même catalogue, when elle s'exécute, then le plan contient zéro mutation et aucun mtime d'asset géré ne change.
- [ ] Given une installation Vercel Skills fixture, when `adopt`, `update` puis `uninstall` s'enchaînent, then le lock legacy est recoverable et aucun asset hors catalogue n'est touché.
- [ ] Given un lock Vercel mixte, when le cycle d'adoption et désinstallation termine, then les entrées hors catalogue et leur lock résiduel restent valides.
- [ ] Given `uninstall --provider` puis `uninstall --all`, when les scénarios s'exécutent, then références, visibilité Codex expliquée, shared Claude et états `retained_unmanaged` correspondent au receipt final.
- [ ] Given chaque étape du cycle, when les sorties humaines et JSON sont capturées, then elles décrivent les mêmes opérations et codes de sortie.

#### US-019: Prouver récupération, concurrence et portabilité

**Description:** As a mainteneur, I want isoler la matrice de défaillance du parcours fonctionnel so that chaque primitive et frontière durable soit testée dans une session dédiée.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-008, US-013, US-015, US-018

**Acceptance Criteria:**

- [ ] Given une fault injection avant et après chaque primitive filesystem, when les scénarios tournent, then chaque cas atteint l'état initial ou `RECOVERY_REQUIRED` avec journal et backups complets.
- [ ] Given des racines sur deux filesystems de test, when une panne traverse leur ordre de commit, then `recover` compense dans l'ordre inverse et aucun succès n'est émis avant le receipt final.
- [ ] Given deux processus concurrents et des signaux à chaque état durable, when les scénarios PTY s'exécutent, then un seul process mute et les codes 130, 143 ou 5 respectent le contrat.
- [ ] Given un subprocess tué par SIGKILL à chaque état durable, when le binaire suivant démarre, then il ne suppose aucun cleanup de signal et atteint l'état `RECOVERY_REQUIRED` attendu avant `recover`.
- [ ] Given assets étrangers, `_shared` mixte, symlinks cassés, modes divergents, permissions refusées et HOME avec espaces Unicode, when la matrice mixed-state s'exécute, then aucune destination étrangère ou ancêtre n'est modifiée.
- [ ] Given `HOME` ou `CODEX_HOME` changé depuis le receipt, when une mutation est tentée, then elle réalise zéro opération et indique comment rétablir les racines d'origine.
- [ ] Given un second Ctrl+C pendant rollback, when le scénario s'exécute, then la compensation continue jusqu'à une borne durable avant la sortie.

#### US-020: Construire et publier les binaires natifs

**Description:** As a nouvel utilisateur, I want exécuter le même binaire natif que celui validé et attesté par la CI so that l'installation ne dépende ni du checkout du repo ni d'une toolchain locale.

**Priority:** P0
**Size:** L (5 pts)
**Dependencies:** Blocked by US-001, US-002, US-003, US-004, US-015, US-018, US-019

**Acceptance Criteria:**

- [ ] Given une release, when cargo-dist construit les artefacts, then il produit archives, checksums, installateur shell et installateur PowerShell pour `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`, `x86_64-apple-darwin`, `aarch64-apple-darwin` et `x86_64-pc-windows-msvc`, avec une baseline macOS 13.0.
- [ ] Given une archive release, when son contenu est inspecté, then elle inclut le binaire, les licences et les métadonnées requises, sans evals, secrets, chemins absolus ni assets runtime externes au binaire.
- [ ] Given chacune des cinq archives release, when elle est extraite sur un runner natif ou compatible avec son OS et son architecture, avec un HOME temporaire sans checkout, Rust, Cargo, Bun ou Node dans le `PATH`, then son propre binaire réussit `--help`, `plan --json` et la vérification du catalogue embarqué; aucune cible ne peut être couverte uniquement par le smoke d'une autre archive.
- [ ] Given une fixture compilée avec un byte catalogue ou manifeste incohérent, when une commande mutante démarre, then elle échoue avant scan utilisateur avec le code d'intégrité documenté.
- [ ] Given la publication d'une version, when la CI cargo-dist s'exécute, then elle vérifie archives et checksums, publie le manifeste externe et une attestation de provenance, puis refuse une release sans attestation créée et vérifiable dans ce gate CI.
- [ ] Given un token GitHub absent, une cible de cross-compilation cassée ou une version/tag incohérent, when la release est demandée, then aucune release partielle ni tag trompeur n'est créé et le blocker exact est retourné.

## Functional Requirements

- FR-01: Le système doit distribuer un catalogue versionné généré depuis `skills/`, `agents/claude`, `agents/codex` et `shared/claude`.
- FR-02: Le manifeste doit enregistrer les chemins relatifs, types, tailles, modes POSIX normalisés et SHA-256 de chaque fichier runtime.
- FR-03: Le système doit exclure `agents/codex/evals` et tout asset de développement du runtime publié.
- FR-04: L'utilisateur doit pouvoir sélectionner l'intégration Claude Code, Codex ou les deux, avec au moins une intégration lors d'un install; cette sélection ne doit pas être présentée comme une frontière de visibilité des skills Codex.
- FR-05: Le système doit installer le catalogue complet de skills sans profil réduit ni sélection individuelle.
- FR-06: Les skills doivent être copiés sous `$HOME/.agents/skills/<name>` comme source canonique unique.
- FR-07: Claude Code doit recevoir par skill un symlink relatif exact `../../.agents/skills/<name>` sous Unix ou une copie gérée byte-identique sous Windows; chaque fichier support est possédé individuellement sous `$HOME/.claude/skills/_shared`, sans ownership automatique du dossier préexistant.
- FR-08: Codex doit découvrir directement `$HOME/.agents/skills` sans copie ni symlink dans son propre dossier; cette visibilité existe tant que le canonique est présent, même si l'intégration agents Codex n'est pas sélectionnée.
- FR-09: Les agents Claude doivent être copiés sous `$HOME/.claude/agents`; les agents Codex sous `${CODEX_HOME:-$HOME/.codex}/agents`.
- FR-10: Les noms, prompts, permissions et modèles publiés doivent être conservés exactement; une incompatibilité locale produit un diagnostic, jamais une substitution.
- FR-11: Les chemins personnels de la machine source doivent être remplacés avant packaging selon une policy contextuelle qui bloque les racines machine-bound et autorise uniquement les scopes utilisateur documentés, les chemins relatifs internes et les commandes `PATH` prévues.
- FR-12: Toutes les commandes doivent utiliser la même registry provider et le même plan d'opérations typé.
- FR-13: `plan` et `--dry-run` doivent être strictement read-only.
- FR-14: Le plan doit distinguer création, mise à jour, suppression, no-op, adoption possible, dérive, conflit, asset retenu et récupération requise.
- FR-15: Un receipt sous `$HOME/.agents/.arthur-workflow/receipt.json` doit enregistrer schéma, versions, état, identités lexicales et réelles des racines, providers, visibilité implicite, assets, destinations, hashes, modes et références.
- FR-16: Le receipt et ses répertoires d'état doivent être créés avec des permissions utilisateur uniquement, soit `0600` pour les fichiers et `0700` pour les dossiers sur les plateformes POSIX.
- FR-17: `adopt`, `install` et `update` doivent valider le schéma v3 connu avant tout transfert, vérifier contenu, mode, type et cible entrée par entrée, archiver chaque lock original sous un chemin non destructif et préserver dans un lock résiduel valide toutes les entrées hors catalogue; identité de noeud, taille, mtime et hash sont revalidés immédiatement avant remplacement, sans promettre un CAS atomique face à Vercel Skills.
- FR-18: Chaque commande mutante doit prendre un verrou interprocessus avant staging.
- FR-19: Chaque racine doit utiliser un staging sur son filesystem; un journal durable doit ordonner préparation, application et commit du receipt, puis permettre compensation et `recover` entre racines.
- FR-20: Le système ne doit jamais écraser, supprimer ou revendiquer automatiquement un chemin préexistant absent du receipt courant sans preuve d'ownership externe vérifiée; un lock Vercel Skills v3 peut autoriser le transfert transactionnel des seuls assets catalogue strictement conformes. Les dossiers partagés préexistants restent non possédés et leurs enfants sont arbitrés fichier par fichier.
- FR-21: Un asset géré mais modifié localement doit être classé `drifted` et bloquer son remplacement ou sa suppression non forcée.
- FR-22: Désinstaller un provider doit retirer uniquement ses activations et agents possédés.
- FR-23: Lors d'un retrait de catalogue, les activations doivent être supprimées avant décrément des références; un skill canonique inchangé ne peut être supprimé qu'à refcount zéro.
- FR-24: Lors du dernier uninstall, un asset dérivé doit rester sur disque comme `retained_unmanaged`, sortir de l'ownership actif et ne jamais devenir une cible destructive ultérieure.
- FR-25: `update` doit réconcilier vers le catalogue embarqué par le CLI invoqué, sans réseau, downgrade ou self-update en v1; l'utilisateur acquiert d'abord le binaire cible.
- FR-26: `status` et `doctor` doivent fonctionner sans réseau et sans mutation, y compris sur receipt partiellement lisible.
- FR-27: Le CLI doit exposer `plan`, `install`, `status`, `doctor`, `update`, `uninstall`, `adopt` et `recover`.
- FR-28: En TTY compatible, Ratatui en `Viewport::Inline` doit fournir sélection provider, avertissement de visibilité Codex, revue du plan, confirmation, progression et résumé persistant dans le scrollback.
- FR-29: En CI, pipe ou mode `--json`, Ratatui, raw mode et les prompts doivent être désactivés.
- FR-30: Un pré-scan de `args_os()` doit activer JSON uniquement pour `--json` situé avant le premier `--`; ce mode doit capturer succès, aide, version, conflit de flags et toute erreur Clap dans l'envelope v1 déterministe, sans sortie automatique Clap ni autre octet sur stdout. `command` et `transaction_id` doivent être nullables avant leur résolution ou allocation.
- FR-31: Le système doit utiliser les codes 0 succès/no-op, 2 usage, 3 conflit/dérive, 4 environnement et 5 transaction, intégrité ou récupération requise.
- FR-32: L'installateur ne doit exécuter aucun script, binaire ou hook contenu dans un skill ou agent.
- FR-33: Les chemins issus du catalogue, de l'environnement et du receipt doivent être validés contre traversal, types inattendus et symlink escape; sous Unix, seule la cible Claude exactement égale à un skill canonique possédé peut traverser entre ces deux racines.
- FR-34: Une seconde application du même catalogue et des mêmes providers doit produire zéro mutation.
- FR-35: L'absence d'un CLI provider doit être signalée mais ne doit pas empêcher une installation préparatoire si ses racines sont sûres.
- FR-36: Le binaire distribué doit être autonome et ne doit dépendre ni d'un checkout, ni de Rust/Cargo, ni d'un runtime JavaScript sur la machine utilisateur.
- FR-37: Copies, mises à jour, diagnostics et rollbacks doivent préserver et vérifier les modes fichiers `0644` et `0755` sans chmod des ancêtres préexistants.
- FR-38: La release v1 doit valider les agents avec Claude Code 2.1.217 et Codex 0.144.6 au minimum; une version locale inférieure rend `doctor` non sain sans altérer les assets.
- FR-39: Toute référence interne de skill ou agent doit résoudre vers un fichier présent dans le catalogue packagé.
- FR-40: Un changement d'identité `HOME`, racine canonique ou `CODEX_HOME` par rapport au receipt doit bloquer les mutations concernées.
- FR-41: GitHub Releases, checksums et l'installateur cargo-dist constituent la racine de distribution runtime; le hash embarqué prouve la cohérence interne. La provenance externe lie commit, version et archives à la CI uniquement comme gate de publication et n'est pas revérifiée offline par le CLI v1.
- FR-42: `--plain` doit couvrir toutes les décisions interactives sans raw mode, cursor addressing, animation ou dépendance à la couleur.
- FR-43: Tout `PathBuf` non UTF-8 issu de l'environnement, du receipt ou du filesystem doit bloquer la mutation avec le code 4; le JSON expose `path_utf8` ou `path_bytes_hex`, mutuellement exclusifs.

## Non-Functional Requirements

- **Performance:** sur une référence Linux x86_64 de 2 vCPU, 7 GB RAM et stockage SSD, mesurée sur 30 runs avec process froid, `plan --json` doit terminer en moins de 250 ms au P95; une fresh install de 50 skills, 6 agents et 4 assets support doit terminer en moins de 2 secondes au P95, hors téléchargement.
- **Responsiveness:** chaque interaction clavier Ratatui doit produire une nouvelle frame en moins de 50 ms au P95 sur le catalogue initial.
- **Security:** 100% des fichiers runtime doivent être vérifiés par SHA-256, type et mode avant mutation; 100% des destinations doivent respecter la policy de racines; 0 script de catalogue exécuté pendant toute commande.
- **Authenticity:** 100% des releases publiques doivent passer le gate CI qui vérifie les checksums cargo-dist et la présence d'une attestation externe liant commit, version et archives; le runtime offline ne revendique aucune vérification de provenance.
- **Privacy:** 0 télémétrie, 0 lecture de credentials provider et 0 émission réseau pendant les commandes filesystem de v1.
- **Accessibility:** 100% des décisions doivent être réalisables au clavier et via `--plain`; le mode plain doit contenir 0 octet ESC et 100% des informations critiques sous forme append-only. Chaque release reçoit un smoke manuel VoiceOver ou Orca sans présenter ce test comme une garantie universelle de lecteur d'écran.
- **Scalability:** sur la même référence, le planificateur doit traiter 500 skills et 50 agents en moins de 750 ms au P95 et sous 64 MB de RSS.
- **Reliability:** une seconde installation identique doit produire 0 opération mutante; 100% des points de panne injectés doivent atteindre l'état initial ou `RECOVERY_REQUIRED` avec 100% des backups encore disponibles.
- **Concurrency:** une seconde commande mutante doit détecter le verrou en moins de 250 ms et réaliser 0 mutation.
- **Portability:** 100% des scénarios critiques doivent passer sur les cinq cibles release Linux musl, macOS x86_64/ARM64 et Windows x86_64 MSVC; toute autre plateforme doit être refusée avant mutation.
- **Maintainability:** le workspace doit conserver au moins 90% de couverture de régions LLVM mesurée par `cargo llvm-cov`; il doit compiler avec 0 warning, 0 bloc `unsafe` et 0 `unwrap()` ou `expect()` de production hors dérogation locale motivée par un invariant validé.
- **Artifact size:** chaque archive compressée du binaire release doit rester sous 25 MB pour le catalogue initial.
- **Compatibility:** les schémas JSON de plan, sortie et receipt doivent rester backward-compatible durant toute la major v1; une version future inconnue doit être refusée sans mutation.

## Edge Cases & Error States

| # | Scenario | Trigger | Expected Behavior | User Message |
|---|----------|---------|-------------------|--------------|
| 1 | HOME absent ou invalide | Environnement minimal, HOME relatif ou inaccessible | Stop avant scan ou création | "Cannot resolve a safe user home. Set an absolute HOME path." |
| 2 | Aucun provider choisi | TTY sans sélection ou CI sans `--provider` | Bloque la confirmation ou retourne usage 2 | "Select at least one provider." |
| 3 | Provider absent ou trop ancien | Binaire absent ou sous la baseline validée | Installe de manière préparatoire, `doctor` non sain | "Provider files can be installed, but runtime compatibility is unverified." |
| 4 | Visibilité Codex implicite | Claude seul sélectionné, Codex présent | Avertit avant confirmation, ne prétend pas isoler les skills | "Codex can discover the canonical skills while they remain installed." |
| 5 | Collision étrangère | Fichier ou dossier homonyme non présent dans le receipt | Classe `conflict`, aucune mutation | "Unmanaged path already exists: <path>. Move it or run adopt after verification." |
| 6 | Contenu identique non possédé | Hash et mode égaux, aucune preuve d'ownership | Classe `adoptable`, n'écrase pas | "Matching unmanaged asset found. Run adopt to transfer ownership." |
| 7 | Symlink cassé ou cyclique | Lien Claude invalide | Classe le type exact, ne suit pas la boucle | "Broken or cyclic symlink detected: <path>." |
| 8 | Symlink escape | Lien hors racine ou cible autre que le canonique exact | Stop intégrité avant mutation | "Symlink location or target is outside the allowed ownership edge." |
| 9 | Asset géré modifié | Hash, type ou mode différent du receipt | Classe `drifted`, bloque update ou uninstall destructif | "Managed asset has local changes and was preserved: <path>." |
| 10 | Receipt corrompu ou futur | JSON invalide ou schema_version supérieur | Read-only limité, mutations refusées | "Installation receipt is unreadable or newer than this CLI." |
| 11 | Lock legacy mixte ou concurrent | Entrées étrangères ou identité changée avant remplacement | Préserve un lock résiduel ou annule l'adoption; concurrence externe résiduelle hors garantie | "Legacy lock changed during adoption. Stop other skill managers and retry." |
| 12 | Deux commandes mutantes | Verrou déjà tenu | Échec immédiat, zéro mutation | "Another Arthur Workflow transaction is running." |
| 13 | Interruption utilisateur | Signal avant ou pendant mutation | Compensation puis code 130 ou 143 | "Installation interrupted. Recovery completed or remains required." |
| 14 | Disque plein ou permission refusée | Échec staging, backup, fsync ou rename | Rollback ou `RECOVERY_REQUIRED` | "Filesystem transaction failed. Run recover after fixing the cause." |
| 15 | Racines multi-filesystem | Canonique et provider sur devices distincts | Stage par racine, journal et compensation ordonnée | "Cross-filesystem transaction is using recoverable commit mode." |
| 16 | Binaire ou catalogue embarqué incohérent | Hash interne différent du manifeste | Runtime bloqué avant scan, code 5 | "Bundled catalog integrity check failed." |
| 17 | Release non attestée | Gate CI sans attestation vérifiable | Publication refusée, aucun contrôle runtime promis | "Release provenance gate failed." |
| 18 | Modèle provider inconnu | Agent exact non supporté localement | `doctor` non sain, fichier inchangé | "Configured model is not recognized by the installed provider version." |
| 19 | Support interne manquant | `_shared` ou autre référence absente du binaire | Compilation ou install bloquée avant copie | "Catalog contains an unresolved internal reference." |
| 20 | Collision `_shared` | Fichier support homonyme non possédé | Identique adoptable, différent conflictuel, autres fichiers préservés | "Shared support file conflicts with an unmanaged file." |
| 21 | Terminal trop étroit ou `TERM=dumb` | Largeur insuffisante ou terminal sans capacités | Layout compact ou renderer plain avec toutes les données | "Plain terminal mode enabled." |
| 22 | Symlink Windows indisponible | Developer Mode désactivé et processus non élevé | Utilise une copie Claude gérée, sans tentative de symlink | "Claude skills use managed copies on Windows." |
| 23 | Dernier provider et skill dérivé | `uninstall --all` avec modifications locales | Conserve comme `retained_unmanaged`, libère l'ownership actif | "Locally modified asset was retained and is no longer managed." |
| 24 | Transaction orpheline | Crash précédent avec journal ou backup | Refuse nouvelle mutation et exige `recover` | "An incomplete transaction requires recover before continuing." |
| 25 | Racine changée | HOME ou CODEX_HOME diffère du receipt | Zéro mutation, retour environnement 4 | "Configured roots differ from the installation receipt." |
| 26 | Chemin Unix non UTF-8 | HOME, destination ou receipt contient des bytes invalides | Refus avant mutation, bytes hex dans le diagnostic | "Non-UTF-8 paths are not supported in v1." |

## Risks & Mitigations

| # | Risk | Probability | Impact | Mitigation |
|---|------|------------|--------|------------|
| 1 | Ratatui inline ne restaure pas certains terminaux après panic ou signal | Med | High | Spike PTY Linux/macOS, panic hook, `try_restore` et renderer plain en US-001 |
| 2 | Un chemin personnel ou une référence support manque | Med | High | Policy contextuelle, inventaire exhaustif et résolution obligatoire des assets `_shared` en US-003 |
| 3 | Les formats d'agents évoluent côté provider | Med | High | Baselines versionnées, validateurs complets et smoke de chargement provider en US-004 |
| 4 | Un symlink traverse une frontière inattendue | Low | High | Validation séparée du lien Claude et de sa cible canonique exacte |
| 5 | Une collision utilisateur est prise pour un asset géré | Low | High | Ownership fondé uniquement sur receipt, adoption explicite et hashes |
| 6 | Une panne multi-filesystem laisse un état mixte | Med | High | Staging par racine, journal fsync, compensation, `recover` et fault injection exhaustive |
| 7 | Vercel Skills et ce CLI mutent simultanément le lock | Med | High | Exclusivité demandée, revalidation inode plus hash, backup, détection d'un lock réapparu et fenêtre TOCTOU documentée |
| 8 | Le modèle exact d'Arthur n'est pas disponible publiquement | Med | Med | Conserver la valeur exacte, l'exposer dans `doctor`, ne jamais choisir à la place de l'utilisateur |
| 9 | Une cible musl ou macOS ne cross-compile pas avec les dépendances retenues | Med | High | Spike host en US-001, matrice cargo-dist et aucune dépendance native non indispensable |
| 10 | Le catalogue volumineux ralentit le startup | Low | Med | Manifeste précompilé, hashes packagés et budgets P95 testés jusqu'à 500 skills |
| 11 | Un asset malveillant tente un traversal ou une exécution | Low | High | Catalogue immuable, validation des chemins, hash plus mode et aucune exécution de contenu |
| 12 | Le produit est perçu comme un gestionnaire généraliste | Med | Med | Positionnement explicite: reproduction opinionated du workflow Arthur, providers limités en v1 |
| 13 | Le hash embarqué est pris pour une preuve d'authenticité | Med | High | Frontière de confiance documentée, checksums cargo-dist et provenance limitée au gate CI |
| 14 | Un changement de HOME cible d'anciens chemins | Low | High | Identité des racines dans le receipt et refus de mutation sur mismatch |
| 15 | Un chemin Unix non UTF-8 casse le contrat JSON | Low | Med | Refus boundary avant mutation et diagnostic lossless `path_bytes_hex` |

## Non-Goals

Explicit boundaries, what this version does NOT include:

- Introduire un runtime ou un outillage JavaScript dans le CLI Rust.
- Installer dans un scope projet tel que `.agents/skills`, `.claude/skills` ou `.codex/agents` du repo courant.
- Supporter les dizaines de providers du CLI Vercel Skills. V1 couvre Claude Code et Codex.
- Distribuer le workflow comme marketplace ou plugin natif Claude/Codex, car cela change les namespaces ou ne couvre pas tous les agents.
- Proposer des profils, un mode minimal ou une sélection skill par skill.
- Utiliser des junctions Windows ou exiger des privilèges administrateur en v1.
- Utiliser un écran alternatif Ratatui ou construire une application terminal full-screen.
- Publier un package crates.io en v1; GitHub Releases et l'installateur cargo-dist sont le canal primaire.
- Introduire Tokio ou un runtime async sans besoin I/O concurrent démontré.
- Installer, mettre à jour, authentifier ou configurer les exécutables Claude Code, Codex, Paneflow ou leurs MCP.
- Modifier `AGENTS.md`, `CLAUDE.md`, `config.toml`, les credentials, sessions, caches, plugins ou bases internes des providers.
- Installer ou exécuter `agents/codex/evals` chez l'utilisateur final.
- Substituer automatiquement un modèle, un prompt, une permission ou un nom incompatible.
- Télécharger un catalogue arbitraire pendant une transaction. Une mise à jour applique uniquement le catalogue embarqué par le CLI invoqué.
- Forcer l'écrasement d'un asset dérivé ou effectuer un downgrade de catalogue en v1.
- Garantir que les skills canoniques sont invisibles à Codex lorsqu'une autre intégration les utilise.
- Garantir l'exclusion atomique d'un gestionnaire de skills externe qui ne partage pas le verrou Arthur Workflow.
- Mettre à jour automatiquement le binaire courant; `update` ne concerne que le catalogue embarqué.
- Ajouter télémétrie, compte utilisateur, backend cloud ou synchronisation distante en v1.

## Files NOT to Modify

- `/home/arthur/.agents/**` - installation personnelle live d'Arthur, interdite aux scripts de développement et tests automatisés; utiliser un HOME temporaire.
- `/home/arthur/.claude/**` - configuration Claude live, interdite aux scripts de développement et tests automatisés.
- `/home/arthur/.codex/**` - configuration et données Codex live, interdits aux scripts de développement et tests automatisés.
- `.codex/hooks.json` - hook local du repo sans rapport avec le produit.
- `.git/**` - métadonnées Git, jamais manipulées par le CLI.
- `agents/codex/evals/**` - suite d'évaluation de développement; elle peut être lue par la validation du catalogue, mais n'est ni déplacée, ni packagée, ni installée dans ce chantier.
- `LICENSE` - licence racine existante; les dépendances nouvelles sont documentées dans `THIRD_PARTY.md` sans réécrire la licence du projet.

## Technical Considerations

- **Architecture:** comment garantir un comportement identique entre TTY, plain, JSON et dry-run? Recommandation: un coeur Rust synchrone `scan -> DesiredState -> Vec<Operation>`, un exécuteur impératif et trois renderers. Engineering doit confirmer que Ratatui et plain ne lisent ni ne mutent le filesystem.
- **Provider semantics:** que signifie sélectionner Codex alors que sa découverte de `$HOME/.agents/skills` est implicite? Recommandation: la sélection gère agents et références d'intégration; l'UI et le receipt exposent séparément `implicit_skill_visibility`.
- **Provider registry:** comment ajouter un provider futur sans branches dispersées? Recommandation: trait Rust scellé en v1 avec `resolve_roots`, `skill_strategy`, `agent_strategy`, `validate_environment` et `describe_operations`; seules Claude et Codex sont enregistrées.
- **Catalog:** faut-il distribuer des fichiers adjacents ou embarquer les assets? Recommandation: `build.rs` refuse tout symlink source, valide les dossiers et fichiers réguliers, puis génère un module de manifeste dont chaque fichier est embarqué avec `include_bytes!`. Cette restriction évite qu'une déréférence de symlink perde le type ou la cible dans l'artefact. `update` applique uniquement la snapshot compilée dans le binaire courant.
- **Shared assets:** où versionner et posséder les documents `_shared` qui ne sont pas des skills publics? Recommandation: source `shared/claude/skills/_shared`, ownership fichier par fichier, coexistence des noms étrangers et suppression du dossier uniquement s'il a été créé par le CLI puis devient vide.
- **Data Model:** JSON receipt ou base locale? Recommandation: structs `serde` versionnées sérialisées dans `$HOME/.agents/.arthur-workflow/receipt.json`, écriture atomique et permissions `0600`; inclure schéma, versions, état, transaction, identités de racines, providers et visibilité implicite.
- **Asset records:** quels champs permettent une preuve complète d'ownership? Recommandation: `kind`, `source_id`, destination lexicale et réelle, `content_hash`, `mode`, `link_target`, `references`, `ownership_state` et version de validateur provider.
- **JSON contract:** quelle envelope évite deux clients incompatibles? Recommandation: pré-scanner `std::env::args_os()` jusqu'au premier `--`, puis produire l'objet v1 obligatoire `{schema_version, command, status, exit_code, catalog_version, transaction_id, providers, summary, operations, diagnostics, data}` pour tout résultat de `Cli::try_parse_from`, y compris aide, version et erreur, sans appeler `clap::Error::exit()`. `--json --plain` devient une erreur JSON de code 2; `--json` après le séparateur n'active rien. `command` est nullable avant résolution et `transaction_id` avant allocation. `status` vaut `success`, `noop`, `blocked`, `failed` ou `recovery_required`; `diagnostics[]` contient `{code, severity, message, path_utf8, path_bytes_hex, remediation}`; les deux champs path sont mutuellement exclusifs; toutes les collections contractuelles sont `Vec` triés ou `BTreeMap`.
- **Operation model:** comment représenter mutations et inverses? Recommandation: enum Rust fermé `EnsureDirectory`, `WriteFile`, `ReplaceFile`, `SetMode`, `CreateSymlink`, `RemoveOwnedPath`, `RewriteLegacyLock` et `WriteReceipt`, avec préconditions, inverse, racine, device et preuve d'ownership.
- **Transactions:** comment couvrir plusieurs filesystems sans promettre une atomicité globale impossible? Recommandation: staging et backups par racine, ordre déterministe, `fsync` du fichier puis du dossier avant chaque transition durable, receipt en dernier, compensation en ordre inverse.
- **Recovery:** comment reprendre après crash sans deviner? Recommandation: journal à états fermés. `recover` rollback toute transaction avant receipt commit; après receipt commit, il termine seulement le cleanup. Une précondition perdue conserve `RECOVERY_REQUIRED` et les backups.
- **Signals:** que peut faire un handler sans violer les invariants Rust ou filesystem? Recommandation: poser uniquement un `AtomicBool`; la boucle principale restaure Ratatui et exécute la compensation à une borne sûre. SIGKILL est couvert exclusivement par le journal et `recover`.
- **Argument parsing:** quelle surface garde les huit commandes typées? Recommandation: Clap 4 derive avec structs par sous-commande, `ValueEnum` pour providers, relations de flags déclaratives et un test `CommandFactory::debug_assert()`.
- **Dependencies:** quelles crates minimales verrouiller? Recommandation: Ratatui 0.30.2 avec Crossterm, Clap 4, `serde`/`serde_json`, `sha2`, `thiserror`, `miette` sans capture de secrets, `fs2`, `signal-hook` et `atomicwrites` sous Windows pour le remplacement durable du journal; `tempfile`, `assert_cmd`, `insta`, `proptest` et une crate PTY Unix en dev; `cargo-llvm-cov` et `cargo-deny` sont des outils CI épinglés. Aucun Tokio.
- **Errors:** comment séparer diagnostics humains et JSON? Recommandation: enums `thiserror` dans le domaine, mapping unique vers codes et diagnostics sérialisables, puis `miette` uniquement dans le renderer humain.
- **Packaging and trust:** comment distinguer cohérence interne et authenticité? Recommandation: cargo-dist génère cinq archives, checksums et installateurs shell et PowerShell depuis `rust-toolchain.toml`; chaque archive passe son smoke sur un runner compatible avec son OS et son architecture, puis GitHub Actions vérifie la provenance. Le CLI offline ne vérifie pas l'attestation et le quickstart épingle un semver exact.
- **Migration:** comment préserver l'ownership Vercel Skills hors catalogue sans prétendre contrôler son processus? Recommandation: exiger l'exclusivité opérationnelle, parser le schéma v3 couvert, archiver l'original, revalider identité de noeud, taille, mtime et hash immédiatement avant rename, puis bloquer `doctor` et toute mutation si un lock concurrent réapparaît. La fenêtre TOCTOU avec un writer non coopératif reste explicitement hors garantie v1.
- **Root identity:** comment réagir si HOME ou CODEX_HOME change? Recommandation: stocker valeurs lexicales, realpaths et identité filesystem disponible; refuser les mutations sur mismatch et demander de rétablir l'environnement original avant uninstall ou migration.
- **Compatibility:** quelles versions provider supporter? Recommandation: Claude Code 2.1.217 et Codex 0.144.6 comme minima initiaux validés, actualisés par release; un provider absent permet la préparation, une version inférieure rend `doctor` non sain.
- **Security:** où appliquer les frontières? Recommandation: valider catalogue, environnement, receipt et chaque destination avant accès; autoriser uniquement l'arête symlink Claude vers le skill canonique exact; n'utiliser aucune commande shell interpolée.
- **Testing:** comment garder les preuves ciblées? Recommandation: unités et property tests sur manifeste, plan et journal; intégration en `TempDir` avec environnement injecté par subprocess; `TestBackend` et horloge/événements injectés; golden tests plain/JSON avec assertion zéro ESC; PTY pour raw mode, resize et signaux; SIGKILL à chaque borne durable; devices distincts pour compensation.

## Success Metrics

| Metric | Baseline (current) | Target | Timeframe | How Measured |
|--------|-------------------|--------|-----------|-------------|
| Couverture du workflow installé | Skills installables, agents manuels | 100% des skills, agents et supports du manifeste pour chaque provider choisi | Month-1 | Matrice des binaires release plus `doctor --json` |
| Nombre d'actions d'onboarding | Plusieurs commandes plus copies manuelles | 2 commandes shell maximum depuis HOME vierge, acquisition du binaire comprise | Month-1 | Test du quickstart versionné |
| Idempotence | Non prouvée sur le workflow complet | 0 mutation au second run dans 100% des scénarios | Month-1 et chaque release | Snapshots filesystem et plans JSON |
| Protection des assets étrangers | Pas d'ownership commun agents plus skills | 0 mutation d'un chemin non possédé | Month-1 et chaque release | Fixtures mixed-state et fault injection |
| Récupération après panne | Aucun rollback complet | 100% des points injectés atteignent l'état initial ou `RECOVERY_REQUIRED` avec backups complets | Month-1 | Suite transactionnelle et `recover` |
| Temps de planification | N/A | P95 inférieur à 250 ms sur 50 skills, 6 agents et 4 assets support | Month-1 | 30 runs sur la machine de référence définie dans les NFR |
| Temps de fresh install | N/A | P95 inférieur à 2 secondes hors téléchargement | Month-1 | Benchmark CI dédié |
| Adoption de l'installation actuelle | Inspection et migration manuelles | 1 plan, 1 confirmation, 0 réécriture des assets identiques | Month-1 | Fixture `.skill-lock.json` v3 |
| Stabilité du contrat automate | N/A | 0 rupture JSON dans la major v1 | Month-6 | Fixtures de schéma versionnées |
| Portabilité | Machine d'Arthur uniquement validée | 100% de la matrice critique verte sur cinq cibles Linux, macOS et Windows | Month-6 | CI cargo-dist sur archives natives |

## Open Questions

- Faut-il signer et notariser les binaires macOS après la première release publique? Owner: Arthur. Deadline: revue Month-1 à partir des retours Gatekeeper. Dépendance: futur durcissement cargo-dist, aucune dépendance pour v1.
- Faut-il publier secondairement `arthur-skills` sur crates.io? Owner: Arthur. Deadline: revue Month-1 après validation du canal binaire. Dépendance: disponibilité du nom et politique de publication, hors scope v1.
- Quel troisième provider mérite un adapter après Claude Code et Codex? Owner: Arthur. Deadline: revue Month-6 à partir des demandes GitHub. Dépendance: futur epic provider, hors scope v1.
[/PRD]
