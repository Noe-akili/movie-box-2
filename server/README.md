# Movie Box — API (backend Node.js)

Serveur Express qui alimente l'interface Movie Box. Aucune dépendance lourde : `express` + `cors`
uniquement, et le `fetch` natif de Node (≥ 18) pour les appels amont.

## Lancer

```bash
npm install          # depuis server/
npm run dev          # redémarrage auto (node --watch)
npm start            # production (sert aussi ../dist si présent)
```

Le serveur lit automatiquement le `.env` du dépôt (racine) et `server/.env`, puis les variables
d'environnement.

| Variable | Défaut | Rôle |
| --- | --- | --- |
| `TMDB_API_KEY` / `VITE_API_KEY` | — | clé TMDB (obligatoire) |
| `PORT` | `5174` | port HTTP |
| `WATCH_REGION` | `FR` | région par défaut pour « où regarder » |
| `ALLOWED_ORIGINS` | `*` | liste d'origines CORS séparées par des virgules |
| `RATE_LIMIT_WINDOW_MS` / `RATE_LIMIT_MAX` | `60000` / `240` | limitation de débit par IP |
| `ARCHIVE_BASE_URL` | `https://archive.org` | source des films libres de droits |
| `DOWNLOADS_DIR` | `server/downloads` | dossier de stockage des téléchargements |
| `DOWNLOAD_CONCURRENCY` | `2` | transferts simultanés (1 à 4) |
| `DOWNLOAD_MAX_BYTES` | `8589934592` (8 Go) | taille maximale d'un fichier |
| `DOWNLOAD_MAX_QUEUE` | `60` | téléchargements conservés au maximum |

## Fonctionnement

- **Cache mémoire avec déduplication** (`src/lib/cache.js`) : une seule requête amont pour plusieurs
  appels simultanés, TTL de 5 min à 6 h selon les données.
- **TMDB** (`src/services/tmdb.js`) : tout le catalogue mondial, métadonnées dans la langue demandée
  (repli automatique sur l'anglais si le synopsis n'est pas traduit), carte normalisée pour le front
  (`id`, `mediaType`, `poster`, `backdrop`, `year`, `rating`…).
- **Internet Archive** (`src/services/archive.js`) : recherche Lucene sur les collections libres
  (`feature_films`, `moviesandfilms`, `classic_tv`, `television`, `Film_Noir`, `SciFi_Horror`,
  `silent_films`, `animationandcartoons`, `classic_cartoons`, `short_films`, `prelinger`,
  `opensource_movies`), puis résolution des fichiers vidéo (priorité MP4/h.264, HD avant SD) avec URL de
  streaming directe et lecteur `<video>` côté client.
- **Filtre de droits** : toute requête Archive ajoute une clause sur `licenseurl`
  (`ARCHIVE_LICENSES` dans `archive.js`). Par défaut `license=free` = domaine public
  (`publicdomain/mark`, `publicdomain/zero`) **ou** licence Creative Commons. Aucune œuvre aux droits non
  déclarés n'entre dans le catalogue gratuit, le lecteur ou les téléchargements. `license=all` reste
  accessible explicitement (avec avertissement dans l'interface) pour fouiller une archive précise.
  Le libellé (`licenseLabel`) est affiché sur chaque fiche, avec le lien vers la licence d'origine.
- **Plateformes légales** (`src/services/providers.js`) : liens vers JustWatch, Pluto TV, Tubi, Plex,
  ARTE, YouTube (bande-annonce) et le catalogue gratuit interne. Les plateformes TMDB
  (`watch/providers`) sont regroupées par type d'offre avec un drapeau `free` pour les offres
  gratuites avec publicité.
- **Robustesse** : timeouts, 2 tentatives sur les appels lents, `Promise.allSettled` sur les rails de
  la page d'accueil (un rail indisponible ne casse jamais la réponse), erreurs JSON homogènes
  `{ error: { status, message } }`.

## Routes

```
GET /api/health
GET /api/legal
GET /api/genres?type=movie|tv
GET /api/home?lang=fr-FR
GET /api/search?q=&type=all|movie|tv&page=&year=
GET /api/discover/movie|tv?genre=&language=&country=&year=&sort=&page=
GET /api/free?collection=&license=free|publicdomain|cc|all&q=&sort=popular|recent|oldest|title&page=&rows=
GET /api/title/movie|tv|archive/:id?lang=&region=
GET /api/title/tv/:id/season/:number
GET /api/downloads
POST /api/downloads
GET /api/downloads/:id
POST /api/downloads/:id/retry
POST /api/downloads/:id/cancel
DELETE /api/downloads/:id
GET /api/downloads/:id/stream
GET /api/downloads/:id/file
```

## Telechargements (services/downloads.js)

Le backend peut **stocker sur son disque** les films et les episodes libres de droits d'Internet Archive,
puis les re-servir en local (lecture hors ligne ou enregistrement sur l'appareil).

- **Validation stricte** : `POST /api/downloads` n'accepte qu'un couple `{ sourceId, fileName }`
  verifie contre les metadonnees Internet Archive (`getArchiveFile`). Aucune URL fournie par le client
  n'est utilisee : le SSRF et les chemins de traverse sont impossibles.
- **File d'attente** : `DOWNLOAD_CONCURRENCY` transferts en parallele (2 par defaut), le reste en attente.
  Les fichiers sont ecrits en `<id>.part` puis renommes une fois complets.
- **Progression** : octets recus, pourcentage, debit (moyenne glissante) et temps restant, pousses toutes
  les secondes dans `GET /api/downloads` (reponse `no-store`, prevue pour du polling court).
- **Reprise** : si un transfert est interrompu (pause, erreur reseau, redemarrage du serveur), le fichier
  `.part` et le manifeste permettent de reprendre a l'octet recu grace a l'en-tete HTTP `Range`.
- **Stockage** : `server/downloads/` (non versionne) contient les fichiers + `manifest.json`.
  `GET /api/downloads/:id/stream` supporte `Range` (lecture dans le lecteur `<video>`),
  `GET /api/downloads/:id/file` force le telechargement (`Content-Disposition: attachment`).

Réponse `/api/title/archive/:id` (extrait) :

```json
{
  "id": "archive-his_girl_friday",
  "mediaType": "archive",
  "playable": true,
  "player": "html5",
  "title": "His Girl Friday",
  "streams": [
    { "name": "his_girl_friday.mp4", "format": "MPEG4", "quality": "HD", "size": 1234567, "url": "https://archive.org/download/…" }
  ],
  "embedUrl": "https://archive.org/embed/his_girl_friday",
  "related": []
}
```

## Légalité

Le serveur ne référence que des sources autorisées : métadonnées TMDB, films et séries du domaine public ou
sous licence libre d'Internet Archive, et liens sortants vers les plateformes officielles. Aucun
scraping de sites pirates, aucun flux protégé relayé — y compris pour les téléchargements, limités aux
fichiers d'Internet Archive validés un par un.

Le catalogue gratuit est en plus **filtre a la source sur les droits declares** (`licenseurl`) : par
defaut, seules les œuvres marquees domaine public ou Creative Commons sont listees, ce qui ecarte
mecaniquement les copies deposees sans autorisation dans les collections ouvertes. Ces mentions restant
declaratives (l'Archive les recoit des depositants), la limite juridique est assumee : sur signalement
d'un ayant droit, l'item est retire et l'identifiant peut etre bloque dans le catalogue.
