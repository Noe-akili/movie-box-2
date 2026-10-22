//! Couche « sources » : une seule porte d'entrée pour les catalogues jouables.
//!
//! Internet Archive, Wikimedia Commons et l'API interne moviebox-internal-api exposent
//! le même contrat de données (carte, flux, licence), si bien que les fiches, le lecteur
//! et les téléchargements fonctionnent sans connaître le fournisseur.

use std::sync::Arc;

use serde_json::{json, Value};

use crate::error::Result;

use super::archive::{
	self, ArchiveSearchParams, ARCHIVE_COLLECTIONS, ARCHIVE_LICENSES,
	DEFAULT_ARCHIVE_COLLECTION, DEFAULT_ARCHIVE_LICENSE,
};
use super::commons::{self, CommonsSearchParams, COMMONS_CATEGORIES, DEFAULT_COMMONS_CATEGORY};
use super::internal;
use super::open;

/// Sources jouables proposées dans l'application.
pub fn free_sources() -> Value {
	let mut sources = vec![
		json!({
			"id": "internal",
			"label": "MovieBox (source principale)",
			"note": "Catalogue et flux vidéo de la source MovieBox locale"
		}),
		json!({
			"id": "archive",
			"label": "Internet Archive",
			"note": "Domaine public & Creative Commons"
		}),
		json!({
			"id": "commons",
			"label": "Wikimedia Commons",
			"note": "Licences vérifiées par la communauté"
		}),
	];
	sources.extend(open::sources());
	Value::Array(sources)
}

/// Source principale : movie-box-2 s'appuie par défaut sur la source MovieBox locale.
pub fn resolve_free_source(value: Option<&str>) -> &'static str {
	if let Some(source) = value.and_then(open::resolve_source) {
		return source;
	}
	match value {
		Some("commons") => "commons",
		Some("archive") => "archive",
		_ => "internal",
	}
}

pub fn is_commons_source(source: &str) -> bool {
	source == "commons"
}

/// Collections / catégories proposées pour une source.
pub fn free_collections(source: &str) -> Value {
	match source {
		"commons" => json!(
			COMMONS_CATEGORIES
				.iter()
				.map(|(id, label)| json!({ "id": id, "label": label }))
				.collect::<Vec<_>>()
		),
		"internal" => json!(internal::COLLECTIONS
			.iter()
			.map(|(id, label)| json!({ "id": id, "label": label }))
			.collect::<Vec<_>>()),
		other if open::resolve_source(other).is_some() => open::collections(other),
		_ => json!(
			ARCHIVE_COLLECTIONS
				.iter()
				.map(|(id, label)| json!({ "id": id, "label": label }))
				.collect::<Vec<_>>()
		),
	}
}

pub fn default_free_collection(source: &str) -> &'static str {
	match source {
		"commons" => DEFAULT_COMMONS_CATEGORY,
		"internal" => internal::DEFAULT_COLLECTION,
		other if open::resolve_source(other).is_some() => open::default_collection(other),
		_ => DEFAULT_ARCHIVE_COLLECTION,
	}
}

pub fn collection_exists(source: &str, collection: &str) -> bool {
	free_collections(source)
		.as_array()
		.map(|items| {
			items
				.iter()
				.any(|item| item.get("id").and_then(|value| value.as_str()) == Some(collection))
		})
		.unwrap_or(false)
}

/// Filtres de droits proposés pour une source.
pub fn free_licenses(source: &str) -> Value {
	match source {
		"commons" => json!([{
			"id": "free",
			"label": "Domaine public & Creative Commons (vérifié par Commons)"
		}]),
		"internal" => json!([{
			"id": "free",
			"label": "Flux fournis par la source MovieBox locale"
		}]),
		other if open::resolve_source(other).is_some() => open::licenses(other),
		_ => json!(
			ARCHIVE_LICENSES
				.iter()
				.map(|(id, label, _)| json!({ "id": id, "label": label }))
				.collect::<Vec<_>>()
		),
	}
}

pub fn resolve_license(source: &str, value: Option<&str>) -> &'static str {
	if source == "archive" {
		archive::resolve_archive_license(value).0
	} else if source == "commons" || source == "internal" || open::resolve_source(source).is_some() {
		"free"
	} else {
		DEFAULT_ARCHIVE_LICENSE
	}
}

pub struct FreeSearchParams<'a> {
	pub source: &'a str,
	pub q: &'a str,
	pub collection: &'a str,
	pub license: &'a str,
	pub page: i64,
	pub rows: i64,
	pub sort: Option<&'a str>,
}

/// Recherche dans la source demandée (archive, commons ou interne).
pub async fn search_free(params: FreeSearchParams<'_>) -> Result<ArcJsonValue> {
	match params.source {
		"commons" => {
			let result = search_commons_inner(params).await?;
			Ok(with_source(result, "commons"))
		}
		"internal" => {
			let result = internal::search(params.q, params.collection, params.page, params.rows).await?;
			Ok(with_source(Arc::new(result), "internal"))
		}
		other if open::resolve_source(other).is_some() => {
			let result = open::search(open::OpenSearchParams {
				source: other,
				q: params.q,
				collection: params.collection,
				page: params.page,
				rows: params.rows,
				sort: params.sort,
			})
			.await?;
			Ok(with_source(result, other))
		}
		_ => {
			let result = search_archive_inner(params).await?;
			Ok(with_source(result, "archive"))
		}
	}
}

type ArcJsonValue = std::sync::Arc<Value>;

fn with_source(mut value: ArcJsonValue, source: &str) -> ArcJsonValue {
	if let Some(object) = Arc::make_mut(&mut value).as_object_mut() {
		object.insert("source".to_string(), Value::String(source.to_string()));
	}
	value
}

async fn search_archive_inner(params: FreeSearchParams<'_>) -> Result<ArcJsonValue> {
	archive::search_archive(ArchiveSearchParams {
		q: params.q,
		collection: params.collection,
		license: params.license,
		page: params.page,
		rows: params.rows,
		sort: params.sort,
		timeout_ms: 10_000,
		retries: 1,
	})
	.await
}

async fn search_commons_inner(params: FreeSearchParams<'_>) -> Result<ArcJsonValue> {
	// `incategory` ne matche que l'appartenance exacte : une recherche libre doit l'ignorer.
	commons::search_commons(CommonsSearchParams {
		q: params.q,
		category: if params.q.is_empty() {
			params.collection
		} else {
			""
		},
		page: params.page,
		rows: params.rows,
	})
	.await
}

/// Fiche complète, quel que soit le fournisseur indiqué par l'identifiant.
///
/// `media_type` reprend le type de la route (`/title/{mediaType}/{id}`) : le front
/// ouvre les fiches internes avec un identifiant nu (`/title/internal/<id>`), sans
/// le préfixe `internal:` — sans cette précision, la fiche partait chez Internet
/// Archive et renvoyait « aucun fichier vidéo lisible ».
pub async fn get_free_title(identifier: &str) -> Result<Value> {
	get_free_title_from(identifier, None).await
}

pub async fn get_free_title_from(identifier: &str, media_type: Option<&str>) -> Result<Value> {
	if open::is_open_id(identifier) {
		open::get_title(identifier).await
	} else if commons::is_commons_id(identifier) {
		commons::get_commons_title(identifier).await
	} else if identifier.starts_with(internal::ID_PREFIX) || media_type == Some("internal") {
		let id = identifier
			.strip_prefix(internal::ID_PREFIX)
			.unwrap_or(identifier);
		internal::title(id).await
	} else {
		archive::get_archive_title(identifier).await
	}
}

/// Validation d'un fichier à télécharger, quel que soit le fournisseur.
pub async fn get_free_file(source_id: &str, file_name: &str) -> Result<Value> {
	if open::is_open_id(source_id) {
		open::get_file(source_id, file_name).await
	} else if commons::is_commons_id(source_id) {
		commons::get_commons_file(source_id, file_name).await
	} else if source_id.starts_with(internal::ID_PREFIX) {
		let rest = &source_id[internal::ID_PREFIX.len()..];
		let mut parts = rest.split(':');
		let sid = parts.next().unwrap_or(rest);
		let season = parts.next().and_then(|value| value.parse::<i64>().ok()).unwrap_or(0);
		let episode = parts.next().and_then(|value| value.parse::<i64>().ok()).unwrap_or(0);
		internal::resolve_download(sid, season, episode, file_name).await
	} else {
		archive::get_archive_file(source_id, file_name).await
	}
}

/// URL distante d'un fichier déjà validé (reprise du téléchargement).
pub fn free_remote_url(source_id: &str, file_name: &str) -> Option<String> {
	if open::is_open_id(source_id) {
		None
	} else if commons::is_commons_id(source_id) {
		commons::commons_remote_url(source_id, file_name)
	} else if source_id.starts_with(internal::ID_PREFIX) {
		None
	} else {
		Some(format!(
			"{}/download/{}/{}",
			crate::config::config().archive_base_url,
			super::providers::urlencode(source_id),
			super::providers::urlencode(file_name)
		))
	}
}

/// Libellé du fournisseur d'un identifiant (messages d'erreur et interface).
pub fn free_source_label(source_id: &str) -> &'static str {
	if open::is_open_id(source_id) {
		open::label(source_id)
	} else if commons::is_commons_id(source_id) {
		"Wikimedia Commons"
	} else if source_id.starts_with(internal::ID_PREFIX) {
		"MovieBox (source principale)"
	} else {
		"Internet Archive"
	}
}
