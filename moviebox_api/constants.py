"""This module stores constant variables"""

import os
import typing as t
from enum import IntEnum, StrEnum
from pathlib import Path

from throttlebuster.constants import (
    DEFAULT_CHUNK_SIZE,
    DEFAULT_READ_TIMEOUT_ATTEMPTS,
    DEFAULT_TASKS_LIMIT,
    DOWNLOAD_PART_EXTENSION,
    DownloadMode,
)

from moviebox_api import logger

"""asyncio event loop"""

MIRROR_HOSTS = (
    # "moviebox.ng",
    "h5.aoneroom.com",
    "movieboxapp.in",
    "moviebox.pk",
    "moviebox.ph",
    "moviebox.id",
    # "fmoviesunblocked.net"
    "v.moviebox.ph",
    "netnaija.video",
    # "sflix.film",
    # "netnaija.com"
)
"""Mirror domains/subdomains of Moviebox"""

ENVIRONMENT_HOST_KEY = "MOVIEBOX_API_HOST"
"""User declares host to use as environment variable using this key"""

SELECTED_HOST = (
    os.getenv(ENVIRONMENT_HOST_KEY) or MIRROR_HOSTS[0]
)  # TODO: Choose the right value based on working status
"""Host adress only with protocol"""

HOST_PROTOCOL = "https"
"""Host protocol i.e http/https"""

HOST_URL = f"{HOST_PROTOCOL}://{SELECTED_HOST}/"
"""Complete host adress with protocol"""

logger.info(f"Moviebox host url - {HOST_URL}")

API_HOST = "h5-api.aoneroom.com"
"""New Moviebox H5 API backend host (no protocol)"""

API_PATH_PREFIX = "/wefeed-h5api-bff/"
"""Path prefix of the new Moviebox H5 API backend"""

NEW_API_HOST_URL = f"{HOST_PROTOCOL}://{API_HOST}/"
"""Complete new Moviebox H5 API backend host address"""

OLD_API_PATH_PREFIX = "/wefeed-h5-bff/"
"""Path prefix of the legacy Moviebox H5 API endpoints"""

DEFAULT_REQUEST_HEADERS = {
    "X-Client-Info": '{"timezone":"Africa/Nairobi"}',  # TODO: Set this value dynamically.
    "Accept-Language": "en-US,en;q=0.5",
    "Accept": "application/json",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64; rv:137.0) Gecko/20100101 Firefox/137.0",
    "Referer": HOST_URL,  # "https://moviebox.ng/movies/titanic-kGoZgiDdff?id=206379412718240440&scene&page_from=search_detail&type=%2Fmovie%2Fdetail",
    "Host": SELECTED_HOST,
    # "X-Source": "",
}
"""For general http requests other than download"""

API_REQUEST_HEADERS = {
    key: value for key, value in DEFAULT_REQUEST_HEADERS.items() if key != "Host"
}
"""Default headers for the new Moviebox H5 API backend.

- The `Host` header of the web host must not be sent to the API host.
- The `Referer` header is required by the new API backend as an origin check.
"""

DOWNLOAD_REQUEST_REFERER = "https://movieboxapp.in/"
"""Referer sent when fetching media/subtitle files from the CDN.

The video CDN (`bcdnxw.hakunaymatata.com`) enforces an anti-hotlink rule that
answers HTTP 429 unless the request carries an allow-listed `Referer`.
`https://movieboxapp.in/` is allow-listed; the previous value
`https://fmoviesunblocked.net/` is not and gets 429 for every media file.
"""

DOWNLOAD_REQUEST_HEADERS = {
    "Accept": "*/*",  # "video/webm,video/ogg,video/*;q=0.9,application/ogg;q=0.7,audio/*;q=0.6,*/*;q=0.5",
    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64; rv:137.0) Gecko/20100101 Firefox/137.0",
    "Origin": SELECTED_HOST,
    "Referer": DOWNLOAD_REQUEST_REFERER,
}
"""For media and subtitle files download requests"""


DownloadQualitiesType: t.TypeAlias = t.Literal[
    "WORST", "BEST", "360P", "480P", "720P", "1080P"
]  # TODO: Add more qualities

DOWNLOAD_QUALITIES = (
    "WORST",
    "BEST",
    "360P",
    "480P",
    "720P",
    "1080P",
)  # TODO: Add more qualities


DEFAULT_CAPTION_LANGUAGE = "English"

DEFAULT_CAPTION_LANGUAGE_SHORT = "en"

CURRENT_WORKING_DIR = Path(os.getcwd())
"""Directory where contents will be saved to by default"""

ITEM_DETAILS_PATH = "/detail"
"""Immediate path to particular item details page"""


DEFAULT_TASKS = 5
"""Default number of connections for download"""

DEFAULT_CONNECT_TIMEOUT: float = 20.0
"""Default HTTP connect timeout in seconds"""

DEFAULT_READ_TIMEOUT: float = 60.0
"""Default HTTP read timeout in seconds"""

DEFAULT_WRITE_TIMEOUT: float = 20.0
"""Default HTTP write timeout in seconds"""

DEFAULT_POOL_TIMEOUT: float = 20.0
"""Default HTTP connection-pool acquire timeout in seconds"""

DEFAULT_CONNECT_RETRIES: int = 5
"""Default number of connection-level retries"""

DEFAULT_RETRY_BACKOFF: float = 0.6
"""Default exponential backoff factor for connection retries"""


class SubjectType(IntEnum):
    """Content types mapped to their integer representatives"""

    ALL = 0
    """Both Movies, series and music contents"""
    MOVIES = 1
    """Movies content only"""
    TV_SERIES = 2
    """TV Series content only"""
    MUSIC = 6
    """Music contents only"""

    UNKNOWN_1 = 5
    """Yet to be known"""

    UNKNOWN = 7
    """Yet to be known"""

    # TODO: Research and update UNKNOWNS

    @classmethod
    def map(cls) -> dict[str, int]:
        """Content-type names mapped to their int representatives"""
        resp = {}
        for entry in cls:
            resp[entry.name] = entry.value
        return resp


class DownloadStatus(StrEnum):
    DOWNLOADING = "downloading"
    FINISHED = "finished"
