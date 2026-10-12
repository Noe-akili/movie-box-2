"""Service JSON compatible avec la source « interne » de movie-box-2.

Expose le contrat attendu par `server-rust/src/services/internal.rs` :

    GET /health
    GET /home        (et /trending, /movies, /anime, /nollywood, /asian,
                      /western, /kids, /music, /education)
    GET /search?q=&page=
    GET /detail/{id}
    GET /episodes/{id}
    GET /stream/{id}?season=&episode=&quality=

Ces endpoints s'appuient sur le paquet `moviebox_api` corrigé (recherche basée
sur le nouveau backend `h5-api.aoneroom.com`). Le fichier est lancé par
`npm run dev:all` du projet movie-box-2 (voir scripts/dev.mjs) sur le port 8000.
"""

import asyncio
import pickle
import threading
from pathlib import Path

from flask import Flask, jsonify, request

from moviebox_api import (
    DownloadableMovieFilesDetail,
    DownloadableTVSeriesFilesDetail,
    MovieDetails,
    Search,
    Session,
    SubjectType,
    Trending,
)
from moviebox_api.exceptions import ZeroSearchResultsError
from moviebox_api.models import SearchResultsItem

app = Flask(__name__)

PER_PAGE = 24

# Rubriques de l'accueil -> terme de recherche (None = rail de tendances).
COLLECTION_QUERIES = {
    "home": None,
    "trending": None,
    "movies": "action",
    "anime": "anime",
    "nollywood": "nollywood",
    "asian": "asian",
    "western": "hollywood",
    "kids": "kids",
    "music": "music",
    "education": "documentary",
}

# Registre persistant : subjectId -> SearchResultsItem (nécessaire pour
# reconstruire l'URL de la fiche et résoudre les flux à partir du seul id).
_ITEMS_FILE = Path(__file__).with_name("._source_items.pkl")
_ITEMS_LOCK = threading.Lock()


def _load_items() -> dict:
    try:
        with _ITEMS_FILE.open("rb") as handle:
            data = pickle.load(handle)
            return data if isinstance(data, dict) else {}
    except Exception:
        return {}


ITEMS: dict = _load_items()


def _persist() -> None:
    try:
        with _ITEMS_FILE.open("wb") as handle:
            pickle.dump(ITEMS, handle)
    except Exception:
        pass


def remember(items) -> None:
    with _ITEMS_LOCK:
        for item in items:
            ITEMS[str(item.subjectId)] = item
        _persist()


def recall(subject_id: str):
    return ITEMS.get(str(subject_id))


def as_card(item) -> dict:
    """Sérialise un SearchResultsItem pour le mappeur `item_to_card` (Rust)."""
    cover = None
    if getattr(item, "cover", None) is not None:
        cover = {"url": str(item.cover.url)}
    return {
        "subjectId": str(item.subjectId),
        "subjectType": int(item.subjectType),
        "title": item.title,
        "description": item.description,
        "releaseDate": item.releaseDate.isoformat() if item.releaseDate else None,
        "duration": item.duration,
        "genre": list(item.genre),
        "cover": cover,
        "countryName": getattr(item, "countryName", ""),
        "imdbRatingValue": getattr(item, "imdbRatingValue", 0),
        "trailer": getattr(item, "trailer", None),
        "detailPath": item.detailPath,
        "hasResource": getattr(item, "hasResource", False),
    }


def run(coro):
    return asyncio.run(coro)


async def with_session(coro_factory):
    session = Session(timeout=30)
    try:
        return await coro_factory(session)
    finally:
        await session._client.aclose()


async def _trending(session):
    model = await Trending(session, per_page=PER_PAGE).get_content_model()
    return model.items


async def _search(session, query, page):
    try:
        model = await Search(
            session, query, subject_type=SubjectType.ALL, page=page, per_page=PER_PAGE
        ).get_content_model()
        return model.items
    except ZeroSearchResultsError:
        return []


async def _listing(session, collection, page):
    query = COLLECTION_QUERIES.get(collection)
    items = await _trending(session) if query is None else await _search(session, query, page)
    return items


async def _detail(session, subject_id):
    item = recall(subject_id)
    if item is None:
        return None
    model = await MovieDetails(item.page_url, session).get_json_details_extractor_model()
    return model.details


async def _episodes(session, subject_id):
    details = await _detail(session, subject_id)
    if details is None:
        return []
    seasons = getattr(getattr(details.resData, "resource", None), "seasons", None) or []
    return [{"se": season.se, "maxEp": season.maxEp} for season in seasons]


async def _streams(session, subject_id, season, episode):
    item = recall(subject_id)
    if item is None:
        return []
    if int(item.subjectType) == int(SubjectType.MOVIES):
        files = await DownloadableMovieFilesDetail(session, item).get_content_model()
    else:
        files = await DownloadableTVSeriesFilesDetail(session, item).get_content_model(
            season, episode
        )
    streams = []
    for media in files.downloads:
        streams.append(
            {
                "url": str(media.url),
                "quality": f"{media.resolution}p",
                "codec": "",
                "size": media.size,
                "duration": 0,
            }
        )
    streams.sort(key=lambda entry: int(entry["quality"].rstrip("p") or 0))
    return streams


@app.get("/health")
def health():
    return jsonify({"status": "ok", "service": "moviebox-source"})


@app.get("/home")
@app.get("/trending")
@app.get("/movies")
@app.get("/anime")
@app.get("/nollywood")
@app.get("/asian")
@app.get("/western")
@app.get("/kids")
@app.get("/music")
@app.get("/education")
def listing():
    collection = request.path.strip("/") or "home"
    page = max(1, request.args.get("page", 1, type=int))
    try:
        items = run(with_session(lambda session: _listing(session, collection, page)))
        remember(items)
        return jsonify({"code": 0, "data": {"items": [as_card(i) for i in items]}})
    except Exception as exc:  # noqa: BLE001
        app.logger.warning("listing %s: %s", collection, exc)
        return jsonify({"code": 1, "message": str(exc), "data": {"list": []}})


@app.get("/search")
def search():
    query = (request.args.get("q") or "").strip()
    page = max(1, request.args.get("page", 1, type=int))
    if not query:
        return jsonify({"code": 0, "data": {"items": []}})
    try:
        items = run(with_session(lambda session: _search(session, query, page)))
        remember(items)
        return jsonify({"code": 0, "data": {"items": [as_card(i) for i in items]}})
    except Exception as exc:  # noqa: BLE001
        app.logger.warning("recherche %r: %s", query, exc)
        return jsonify({"code": 1, "message": str(exc), "data": {"items": []}})


@app.get("/detail/<subject_id>")
def detail(subject_id: str):
    try:
        details = run(with_session(lambda session: _detail(session, subject_id)))
    except Exception as exc:  # noqa: BLE001
        app.logger.warning("détail %s: %s", subject_id, exc)
        return jsonify({"code": 1, "message": str(exc), "data": {}})
    if details is None:
        return jsonify({"code": 1, "msg": "not found", "data": {}})

    sub = details.resData.subject
    stars = getattr(details.resData, "stars", None) or []
    data = {
        "subjectId": str(sub.subjectId),
        "subjectType": int(sub.subjectType),
        "title": sub.title,
        "description": sub.description,
        "releaseDate": sub.releaseDate.isoformat() if sub.releaseDate else None,
        "duration": sub.duration,
        "genre": list(sub.genre),
        "cover": {"url": str(sub.cover.url)} if sub.cover else None,
        "imdbRatingValue": getattr(sub, "imdbRatingValue", 0),
        "detailPath": sub.detailPath,
        "staffList": [
            {
                "name": getattr(star, "name", None),
                "avatarUrl": str(star.avatarUrl) if getattr(star, "avatarUrl", None) else None,
            }
            for star in stars
        ],
    }
    return jsonify({"code": 0, "data": data})


@app.get("/episodes/<subject_id>")
def episodes(subject_id: str):
    try:
        seasons = run(with_session(lambda session: _episodes(session, subject_id)))
    except Exception as exc:  # noqa: BLE001
        app.logger.warning("épisodes %s: %s", subject_id, exc)
        return jsonify({"code": 1, "message": str(exc), "data": {"seasons": []}})
    return jsonify({"code": 0, "data": {"seasons": seasons}})


@app.get("/stream/<subject_id>")
def stream(subject_id: str):
    season = request.args.get("season", 1, type=int)
    episode = request.args.get("episode", 1, type=int)
    try:
        streams = run(
            with_session(lambda session: _streams(session, subject_id, season, episode))
        )
    except Exception as exc:  # noqa: BLE001
        app.logger.warning("flux %s: %s", subject_id, exc)
        return jsonify({"code": 1, "message": str(exc), "data": {"streamList": []}})
    return jsonify({"code": 0, "data": {"streamList": streams}})


if __name__ == "__main__":
    app.run(host="0.0.0.0", port=8000, threaded=True)
