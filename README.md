# 🎬 Movie Box

Application **web complète** (frontend Vue 3 + backend Rust / Axum) dans le style de l'application
MovieBox : catalogue mondial de films, séries et animés, **lecture gratuite des films libres de
droits**, bandes-annonces officielles et orientation vers les offres légales gratuites.

## ✨ Ce que fait l'application

| Fonction | Détail |
| --- | --- |
| Catalogue mondial | Tous les films / séries / animés via TMDB : recherche, genres, langues (FR, EN, ES, HI, KO, JA…), tri, chargement infini |
| Films gratuits | Catalogue libre de droits (Internet Archive) **filtré à la source sur les droits déclarés** (domaine public / Creative Commons) : longs-métrages, séries TV classiques, cinéma noir, muets, animation, courts-métrages, archives Prelinger — **lus directement dans l'application** (lecteur HTML5) |
| Bande-annonce | Bande-annonce YouTube officielle (TMDB) pour les œuvres encore protégées |
| Où regarder | Plateformes légales de la région choisie, en mettant en avant les offres **gratuites avec publicité** (Pluto TV, Tubi, Plex, ARTE…) + JustWatch |
| Ma liste | Favoris et historique stockés en local (aucun compte, aucune donnée envoyée) |
| Téléchargement | Films et **épisodes de séries libres de droits** téléchargés et stockés par le backend : progression en direct, reprise après interruption, lecture hors ligne et enregistrement sur l'appareil |
| Langues | Interface en français, métadonnées dans 5 langues, région des offres configurable |
| Mobile-first / PWA | Barre de navigation basse, gestes, safe-area iOS, installable sur l'écran d'accueil |

## 🧱 Architecture

```
movie-box-2/
├── index.html                 # entrée Vite
├── src/                       # FRONTEND (Vue 3 + Vue Router + Tailwind)
│   ├── api/client.js          # client HTTP unique (/api), langue + région automatiques
│   ├── components/            # TopBar, BottomNav, HeroCarousel, MediaRow, PosterCard, DownloadButton, DownloadRow…
│   ├── pages/                 # Home, Browse (films/séries), Free, Search, Detail, Watch, Library, Downloads, Profile
│   ├── stores/                # settings, library (favoris/historique), downloads (progression), recentSearches, toast, loading
│   └── util/                  # formatage (durées, tailles, débits, dates), routage des médias
├── server-rust/               # BACKEND (Rust + Axum)
│   ├── downloads/             # fichiers téléchargés + manifest.json (non versionné)
│   └── src/
│       ├── http.rs, lib.rs    # serveur Axum partagé (binaire autonome + Tauri)
│       ├── routes/api.rs      # /api/home, /api/search, /api/discover, /api/free, /api/title…
│       ├── routes/downloads.rs# /api/downloads (lancer, suivre, lire, supprimer)
│       ├── services/tmdb.rs   # catalogue mondial (cache mémoire + repli anglais)
│       ├── services/archive.rs# films et séries libres de droits (Internet Archive) + détection des épisodes
│       ├── services/commons.rs# films libres (Wikimedia Commons)
│       ├── services/moviebox.rs # source MovieBox portée nativement (remplace le service Python)
│       ├── services/internal.rs # source interne (moviebox)
│       ├── services/downloads.rs # stockage local, progression, reprise (HTTP Range), file d'attente
│       └── services/providers.rs # plateformes légales (dont offres gratuites)
├── src-tauri/                 # APPLICATION ANDROID/DESKTOP (Tauri v2)
│   ├── src/                   # lance le serveur Rust embarqué + commande `server_origin`
│   ├── build.rs               # intègre la clé TMDB du .env dans le binaire
│   └── tauri.conf.json        # configuration (identifier com.moviebox.app, frontendDist ../dist)
├── .github/workflows/android.yml # build de l'APK (tout est géré par GitHub)
├── scripts/dev.mjs            # npm run dev:all (API Rust + front en une seule commande)
├── scripts/patch-android-manifest.mjs # autorise le cleartext vers le serveur embarqué
└── design/                    # captures de référence (non versionné)
```

## 🚀 Démarrage

```bash
# 1. dépendances (frontend + backend)
npm run install:all

# 2. clé TMDB dans .env (déjà présente dans le dépôt)
#    VITE_API_KEY="ta-cle-tmdb"     ou     TMDB_API_KEY="ta-cle-tmdb"

# 3. lancer l'API + l'interface en une commande
npm run dev:all
#    frontend → http://localhost:5173
#    backend  → http://localhost:5174/api/health

# ou séparément
npm run dev        # frontend seul (proxy /api → http://localhost:5174)
npm run dev:api    # backend seul
```

Mise en production (l'API Rust sert aussi l'interface compilée) :

```bash
npm run build      # génère dist/
npm start          # cargo run --release (http://localhost:5174, interface + API)
```

Variables d'environnement (fichier `.env` à la racine) :

| Variable | Défaut | Rôle |
| --- | --- | --- |
| `TMDB_API_KEY` / `VITE_API_KEY` | — | clé TMDB (obligatoire pour le catalogue) |
| `PORT` | `5174` | port du serveur Rust |
| `WATCH_REGION` | `FR` | région par défaut des offres légales |
| `ALLOWED_ORIGINS` | `*` | origines CORS autorisées |
| `RATE_LIMIT_MAX` | `240` | requêtes par minute et par IP |
| `DOWNLOADS_DIR` | `server-rust/downloads` | dossier de stockage des fichiers téléchargés |
| `DOWNLOAD_CONCURRENCY` | `2` | téléchargements simultanés (1 à 4) |
| `DOWNLOAD_MAX_BYTES` | `8589934592` (8 Go) | taille maximale d'un fichier téléchargeable |
| `DOWNLOAD_MAX_QUEUE` | `60` | nombre maximal de téléchargements conservés |

## 📱 Android (APK)

L'application peut être empaquetée en **APK Android** via [Tauri v2](https://v2.tauri.app/).
Le backend Rust est **embarqué dans le processus de l'app** : au démarrage, il écoute sur
`http://127.0.0.1:5174` et l'interface (WebView) l'appelle directement. **Aucun service
Python n'est nécessaire** : la source MovieBox est portée nativement en Rust
(`server-rust/src/services/moviebox.rs`).

La compilation est **entièrement déléguée à GitHub Actions**
(`.github/workflows/android.yml`) : JDK, SDK/NDK Android, cibles Rust et dépendances npm sont
installés par le workflow. **Rien n'est requis en local.**

1. pousser sur `main` (ou lancer le workflow *Build Android APK* manuellement) ;
2. ouvrir l'artefact **`movie-box-android-debug`** de l'exécution ;
3. installer `app-<abi>-debug.apk` sur le téléphone (APK **debug déjà signé**, sideload
   autorisé au besoin : *Paramètres → Sécurité → Sources inconnues*).

Détails de l'environnement de build (épinglés dans le workflow) : AGP `9.3.1`, Gradle `9.5.0`,
NDK `28.2.13676358`, JDK 17, `compileSdk`/`targetSdk` 37, `minSdk` 21, cible `aarch64`.

Points spécifiques à Android :

- le manifeste est patché par `scripts/patch-android-manifest.mjs` pour autoriser le
  **cleartext** (`android:usesCleartextTraffic="true"`) vers `127.0.0.1` (bloqué par défaut
  depuis Android 9) ;
- la clé TMDB du `.env` est **intégrée au binaire** à la compilation
  (`src-tauri/build.rs` → `cargo:rustc-env`), car aucun `.env` n'existe sur l'appareil ;
- le frontend résout dynamiquement l'origine du serveur embarqué
  (`src/main.js` → `server_origin`, puis `src/api/client.js` → `apiBase()` / `mediaUrl()`),
  y compris pour les balises `<video>` et les liens de téléchargement.

## 🔌 API (backend Rust)

| Route | Description |
| --- | --- |
| `GET /api/health` | état du serveur, du cache et de la clé TMDB |
| `GET /api/home?lang=fr-FR` | bannière + rails (tendances, films, séries, anime, K-drama, Bollywood… + rails gratuits) |
| `GET /api/genres?type=movie\|tv` | genres traduits |
| `GET /api/search?q=&type=all\|movie\|tv&page=` | recherche TMDB + correspondances gratuites |
| `GET /api/discover/movie\|tv?genre=&language=&sort=&page=` | catalogue complet paginé |
| `GET /api/free?collection=&license=free\|publicdomain\|cc\|all&q=&sort=&page=` | catalogue libre de droits (Internet Archive), filtré sur les droits déclarés |
| `GET /api/title/movie\|tv\|archive/:id` | fiche complète : crédits, bande-annonce, images, saisons, plateformes légales, flux vidéo |
| `GET /api/title/tv/:id/season/:n` | épisodes d'une saison |
| `GET /api/legal` | explication du modèle de sources |
| `GET /api/downloads` | téléchargements + progression + état du stockage |
| `POST /api/downloads` | lance le téléchargement d'un film ou d'un épisode (`sourceId`, `fileName`, `kind`…) |
| `GET /api/downloads/:id` | détail d'un téléchargement |
| `POST /api/downloads/:id/retry` | reprend un transfert (repart de l'octet déjà reçu) |
| `POST /api/downloads/:id/cancel` | met en pause un transfert en cours |
| `DELETE /api/downloads/:id` | supprime le téléchargement et son fichier |
| `GET /api/downloads/:id/stream` | lecture du fichier local (supporte `Range`, pour le lecteur intégré) |
| `GET /api/downloads/:id/file` | enregistrement du fichier sur l'appareil (`Content-Disposition: attachment`) |

## 📥 Téléchargements (stockage par le backend)

1. Sur la fiche ou la page de lecture d'une œuvre **libre de droits** (film ou série Internet Archive),
   le bouton « Télécharger » envoie au backend l'identifiant d'archive et le **nom exact du fichier vidéo**.
2. Le backend vérifie ce couple contre les métadonnées Internet Archive (aucune URL cliente n'est acceptée),
   applique la limite de taille puis télécharge le fichier dans `server-rust/downloads/` dans une file limitée
   (2 transferts simultanés par défaut).
3. La progression (octets reçus, pourcentage, débit, temps restant) est exposée par `GET /api/downloads` ;
   l'onglet **Téléchargements** la rafraîchit toutes les 1,2 s et affiche un badge dans la barre de navigation.
4. Un manifeste (`server-rust/downloads/manifest.json`) + un fichier `.part` permettent de **reprendre** un transfert
   interrompu (même après un redémarrage du serveur) grâce aux requêtes `Range`.
5. Un fichier terminé se lit **hors ligne** dans le lecteur intégré (`/watch/local/:id`) ou s'enregistre sur
   l'appareil (`GET /api/downloads/:id/file`).

Les archives multi-épisodes sont détectées automatiquement : `S01E02`, `1x02`, `Épisode 3`, `001. …`,
`Série - 001 - Titre` et les dossiers `Season 4/` deviennent des épisodes numérotés
(voir « Séries TV classiques » dans le catalogue gratuit).

## ⚖️ Modèle de sources (important)

Movie Box **n'héberge ni ne diffuse d'œuvre protégée par le droit d'auteur**. L'application :

1. **référence** l'intégralité du catalogue mondial (métadonnées TMDB, affiches, bandes-annonces officielles) ;
2. **lit en streaming** uniquement les films **libres de droits** (domaine public / licences libres)
   fournis légalement par Internet Archive, avec le lecteur intégré ;
3. **oriente** vers les plateformes officielles pour les œuvres protégées, en priorisant les offres
   **gratuites financées par la publicité** (Pluto TV, Tubi, Plex, ARTE, JustWatch…) ;
4. **peut stocker localement** (dossier `server-rust/downloads/`) ces mêmes œuvres libres de droits, à la demande de
   l'utilisateur, pour une lecture hors ligne. Aucune œuvre protégée n'est jamais téléchargée ni relayée ;
5. **filtre les droits à la source** : le catalogue gratuit interroge Internet Archive avec une clause sur
   `licenseurl` (domaine public ou licence Creative Commons), actif par défaut et affiché sur chaque fiche,
   au lieu d'agréger des liens collectés ailleurs.

C'est ce qui permet à l'application de rester **gratuite, durable et conforme** : les sources pirates
(scraping de liens, agrégateurs illégaux) ne sont pas intégrées, car elles exposent à des poursuites
et à un blocage rapide du service.

## 🎨 Design

L'interface reprend la charte MovieBox : fond `#07070B`, accent rouge / rose, jaquettes 2:3,
bannière plein écran, rails défilants, barre de navigation basse.
Les captures de référence se déposent dans `design/` (voir `design/README.md`) pour affiner
pixel par pixel.

## 🧪 Vérifications rapides

```bash
curl -s localhost:5174/api/health
curl -s "localhost:5174/api/search?q=matrix" | head -c 300
npm run build       # build de production du frontend
```
