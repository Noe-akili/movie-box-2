//! Liens vers les plateformes légales (y compris les offres gratuites avec publicité).

use serde_json::{json, Value};

use crate::config::config;

pub fn build_legal_links(title: &str, year: Option<i64>, media_type: &str, tmdb_id: Option<i64>) -> Value {
	if title.is_empty() {
		return json!([]);
	}

	let query = match year {
		Some(year) => format!("{title} {year}"),
		None => title.to_string(),
	};
	let is_tv = media_type == "tv";

	json!([
		{
			"id": "justwatch",
			"name": "JustWatch",
			"description": "Comparateur officiel : trouve l'offre gratuite ou incluse pour ce titre",
			"url": format!("https://www.justwatch.com/fr/recherche?q={}", urlencode(title)),
			"free": false,
			"icon": "search",
		},
		{
			"id": "pluto",
			"name": "Pluto TV",
			"description": "Films et séries gratuits financés par la publicité",
			"url": format!("https://pluto.tv/fr/search?q={}", urlencode(title)),
			"free": true,
			"icon": "tv",
		},
		{
			"id": "tubi",
			"name": "Tubi",
			"description": "Catalogue gratuit avec publicité",
			"url": format!("https://tubitv.com/search/{}", urlencode(&query)),
			"free": true,
			"icon": "tv",
		},
		{
			"id": "plex",
			"name": "Plex",
			"description": "Films gratuits avec publicité",
			"url": format!("https://watch.plex.tv/search?query={}", urlencode(title)),
			"free": true,
			"icon": "tv",
		},
		{
			"id": "arte",
			"name": "ARTE",
			"description": "Films et documentaires gratuits (service public européen)",
			"url": format!("https://www.arte.tv/fr/search/?q={}", urlencode(title)),
			"free": true,
			"icon": "tv",
		},
		{
			"id": "youtube",
			"name": "Bande-annonce sur YouTube",
			"description": "Extraits et bandes-annonces officielles",
			"url": format!(
				"https://www.youtube.com/results?search_query={}",
				urlencode(&format!(
					"{} {} bande annonce officielle",
					query,
					if is_tv { "série" } else { "film" }
				))
			),
			"free": true,
			"icon": "play",
		},
		{
			"id": "archive",
			"name": "Internet Archive",
			"description": "Films du domaine public lisibles directement dans Movie Box",
			"url": format!("/free?q={}", urlencode(title)),
			"free": true,
			"icon": "archive",
			"internal": true,
		},
		{
			"id": "tmdb",
			"name": "Fiche TMDB",
			"description": "Source de référence des métadonnées du film",
			"url": match tmdb_id {
				Some(tmdb_id) => format!(
					"https://www.themoviedb.org/{}/{}",
					if is_tv { "tv" } else { "movie" },
					tmdb_id
				),
				None => format!("https://www.themoviedb.org/search?query={}", urlencode(title)),
			},
			"free": true,
			"icon": "info",
			"externalNote": true,
		},
	])
}

pub fn legal_info() -> Value {
	json!({
		"region": config().watch_region,
		"statement":
			"Movie Box indexe l'ensemble du catalogue cinématographique mondial et lit les films libres de droits (Internet Archive). \
Pour les films sous copyright, l'application renvoie vers les plateformes officielles, dont les offres gratuites avec publicité.",
		"archiveFilter":
			"Les films et séries gratuits sont filtrés sur les droits déclarés à la source : domaine public \
(Creative Commons mark/zero) ou licence Creative Commons. Le catalogue gratuit est browsé par collection \
et par filtre de droits, jamais par recherche de liens externes.",
	})
}

/// encodeURIComponent (JS) : échappement strict pour les URL de recherche.
pub fn urlencode(value: &str) -> String {
	let mut encoded = String::with_capacity(value.len() * 3);
	for byte in value.bytes() {
		match byte {
			b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
				encoded.push(byte as char)
			}
			_ => encoded.push_str(&format!("%{byte:02X}")),
		}
	}
	encoded
}
