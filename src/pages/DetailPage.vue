<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { fetchSeason, fetchTitle } from "../api/client";
import AppIcon from "../components/AppIcon.vue";
import DownloadButton from "../components/DownloadButton.vue";
import EmptyState from "../components/EmptyState.vue";
import MediaGrid from "../components/MediaGrid.vue";
import SkeletonRail from "../components/SkeletonRail.vue";
import { useLibrary } from "../stores/library";
import { useSettings } from "../stores/settings";
import { toast } from "../stores/toast";
import { formatCount, formatDate, formatRuntime, formatSize } from "../util/format";
import { isFree, isSeriesTitle, mediaTypeLabel, seasonCountOf, watchRoute } from "../util/media";

const props = defineProps({
	mediaType: { type: String, required: true },
	id: { type: String, required: true },
});

const { isFavorite, toggleFavorite, addToHistory } = useLibrary();
const settings = useSettings();
const route = useRoute();
const router = useRouter();

// Raccourci depuis le dossier d'une série (« Épisodes manquants ») : on ouvre
// l'assistant et on pré-sélectionne les épisodes non possédés.
const autoMissing = ref(route.query.download === "missing");
const automaticSeason = ref(
	Number.isFinite(Number(route.query.season)) ? Number(route.query.season) : null,
);

const title = ref(null);
const isLoading = ref(true);
const errorMessage = ref(null);
const selectedSeason = ref(null);
const episodes = ref([]);
const isLoadingEpisodes = ref(false);

const free = computed(() => isFree(title.value));
const saved = computed(() => (title.value ? isFavorite(title.value) : false));
const isSeries = computed(() => isSeriesTitle(title.value));
// Une archive libre de droits (ou un titre interne) peut contenir toute une série :
// chaque fichier devient un épisode. On s'appuie sur `kind`/`episodes` réels de l'API
// pour ne jamais masquer la section à cause d'un seul épisode détecté.
const archiveEpisodes = computed(() => {
	// `episodes` peut être un compte sur certaines sources : seul le tableau compte.
	const list = Array.isArray(title.value?.episodes) ? title.value.episodes : [];
	if (list.length > 1) return list;
	return list.length === 1 && isSeries.value ? list : [];
});

const mainChips = computed(() => {
	const item = title.value;
	if (!item) return [];

	// `seasons` peut etre un compte (TMDB) ou le tableau des saisons : ne jamais concatener le tableau.
	const seasonCount = seasonCountOf(item);

	return [
		item.year ? String(item.year) : null,
		isSeries.value && seasonCount ? `${seasonCount} saison${seasonCount > 1 ? "s" : ""}` : formatRuntime(item.runtime),
		item.ageRating ? `-${item.ageRating}` : null,
		item.rating ? `★ ${item.rating.toFixed(1)}${item.votes ? ` (${formatCount(item.votes)})` : ""}` : null,
	].filter(Boolean);
});

const episodeMeta = (episode) => {
	const streams = episode.streams || [];
	const best = streams[0] || {};
	return [best.quality, best.format, formatSize(best.size), streams.length ? `${streams.length} fichier(s)` : null]
		.filter(Boolean)
		.join(" • ");
};

const freeProviders = computed(() => (title.value?.providers?.groups || []).filter((group) => group.free));
const paidProviders = computed(() => (title.value?.providers?.groups || []).filter((group) => !group.free));
const legalLinks = computed(() => title.value?.legalLinks || []);
const externalLinks = computed(() => title.value?.externalLinks || []);

const details = computed(() => {
	const item = title.value;
	if (!item || free.value) return [];

	return [
		{ label: "Réalisation", value: item.credits?.directors?.join(", ") },
		{ label: "Scénario", value: item.credits?.writers?.join(", ") },
		{ label: "Studios", value: (item.studios || []).join(", ") },
		{ label: "Diffusion", value: (item.networks || []).join(", ") },
		{ label: "Pays", value: (item.countries || []).join(", ") },
		{ label: "Langues", value: (item.spokenLanguages || []).join(", ") },
		{ label: "Sortie", value: formatDate(item.releaseDate) },
		{ label: "Statut", value: item.status },
	].filter((entry) => Boolean(entry.value));
});

const loadEpisodes = async (seasonNumber) => {
	if (!title.value?.tmdbId || !seasonNumber) return;

	selectedSeason.value = seasonNumber;
	isLoadingEpisodes.value = true;

	try {
		const data = await fetchSeason(title.value.tmdbId, seasonNumber);
		episodes.value = data.episodes || [];
	} catch {
		episodes.value = [];
	} finally {
		isLoadingEpisodes.value = false;
	}
};

const load = async () => {
	isLoading.value = true;
	errorMessage.value = null;
	title.value = null;
	episodes.value = [];
	selectedSeason.value = null;

	try {
		const data = await fetchTitle(props.mediaType, props.id);
		title.value = data;
		addToHistory(data);

		if (data.mediaType === "tv" && data.seasons?.length) {
			await loadEpisodes(data.seasons[0].seasonNumber);
		}
	} catch (error) {
		errorMessage.value = error.message;
	} finally {
		isLoading.value = false;
	}
};

const onToggleFavorite = () => {
	if (!title.value) return;
	const added = toggleFavorite(title.value);
	if (added) toast.success(`« ${title.value.title} » ajouté à Ma liste`);
	else toast.info(`« ${title.value.title} » retiré de Ma liste`);
};

const onShare = async () => {
	const url = window.location.href;

	try {
		if (navigator.share) {
			await navigator.share({ title: title.value?.title || "Movie Box", url });
			return;
		}

		await navigator.clipboard.writeText(url);
		toast.success("Lien copié dans le presse-papiers");
	} catch {
		toast.info(url);
	}
};

onMounted(() => {
	load();
	if (autoMissing.value) {
		const query = { ...route.query };
		delete query.download;
		delete query.season;
		router.replace({ query });
	}
});
watch(() => [props.mediaType, props.id], load);
watch(
	() => settings.value.language,
	() => {
		if (title.value) load();
	},
);
</script>


<template>
	<div class="pb-nav">
		<SkeletonRail v-if="isLoading" :count="6" :title="false" variant="row" />

		<EmptyState
			v-else-if="errorMessage"
			icon="info"
			title="Titre indisponible"
			:description="errorMessage"
			action-label="Réessayer"
			@action="load"
		/>

		<template v-else-if="title">
			<!-- En-tête visuel -->
			<section class="relative isolate">
				<div
					class="absolute inset-0 -z-10 h-[62%] bg-cover bg-center"
					:style="title.backdrop ? { backgroundImage: `url(${title.backdrop})` } : {}"
				>
					<div class="h-full w-full bg-hero-gradient"></div>
					<div class="h-full w-full bg-hero-scrim"></div>
				</div>

				<div class="page-top-xl mx-auto max-w-[1800px] px-4 md:px-6 lg:px-8">
					<div class="flex flex-col gap-5 sm:flex-row sm:gap-7">
						<img
							v-if="title.poster"
							:src="title.poster"
							:alt="title.title"
							class="w-32 shrink-0 rounded-2xl ring-1 ring-white/10 shadow-deck sm:w-40 md:w-48"
						/>

						<div class="space-y-4">
							<div>
								<h1 class="text-2xl font-bold leading-tight tracking-tight sm:text-3xl md:text-4xl">
									{{ title.title }}
								</h1>
								<p v-if="title.originalTitle && title.originalTitle !== title.title" class="text-sm text-white/40">
									{{ title.originalTitle }}
								</p>
								<p v-if="title.tagline" class="mt-1 text-sm italic text-brandSoft">{{ title.tagline }}</p>
							</div>

							<div class="flex flex-wrap items-center gap-2">
								<span
									v-for="chip in mainChips"
									:key="chip"
									class="badge bg-white/10 text-white/75 backdrop-blur"
									:class="chip.startsWith('★') ? 'text-gold' : ''"
								>
									{{ chip }}
								</span>
								<span v-if="free" class="badge badge-free">
									<AppIcon name="gift" :size="11" />
									Gratuit &amp; légal
								</span>
								<span v-if="title" class="badge bg-white/10 text-white/75 backdrop-blur">
									{{ mediaTypeLabel(title) }}
								</span>
							</div>

							<div v-if="title.genres?.length" class="flex flex-wrap gap-2">
								<span v-for="genre in title.genres" :key="genre" class="chip !cursor-default !py-1 text-xs">
									{{ genre }}
								</span>
							</div>

							<div class="flex flex-wrap items-center gap-2.5 pt-1">
								<RouterLink :to="watchRoute(title)" class="btn btn-solid px-6 py-3">
									<AppIcon name="play" :size="18" filled />
									<span>{{ free ? "Regarder maintenant" : "Où regarder gratuitement" }}</span>
								</RouterLink>

								<!-- Le bouton reste affiché même quand les flux sont attachés
									     aux épisodes (séries gratuites) : l'assistant saura les
									     résoudre saison par saison. -->
								<DownloadButton
									v-if="free && (isSeries || title.streams?.length || archiveEpisodes.length)"
									:item="title"
									:stream="title.streams?.[0] || null"
									label="Télécharger"
									:auto-open="autoMissing"
									:auto-missing="autoMissing"
									:initial-season="automaticSeason"
								/>

								<button type="button" class="btn btn-inverse px-4 py-3" @click="onToggleFavorite">
									<AppIcon name="heart" :size="18" :filled="saved" />
									<span>{{ saved ? "Dans Ma liste" : "Ma liste" }}</span>
								</button>

								<button type="button" class="icon-btn" aria-label="Partager" @click="onShare">
									<AppIcon name="share" :size="18" />
								</button>
							</div>
						</div>
					</div>
				</div>
			</section>

			<!-- Synopsis + informations -->
			<section class="mx-auto max-w-[1800px] space-y-6 px-4 pt-8 md:px-6 lg:px-8">
				<div class="grid gap-6 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
					<div class="space-y-3">
						<h2 class="section-title">Synopsis</h2>
						<p class="text-sm leading-relaxed text-white/70">
							{{ title.overview || "Aucun synopsis disponible dans la langue choisie." }}
						</p>

						<ul v-if="title.subjects?.length" class="flex flex-wrap gap-2 pt-1">
							<li v-for="subject in title.subjects" :key="subject" class="chip !cursor-default !py-1 text-xs">
								{{ subject }}
							</li>
						</ul>
					</div>

					<dl v-if="details.length" class="surface space-y-2.5 p-4 text-sm">
						<div v-for="entry in details" :key="entry.label" class="flex gap-3">
							<dt class="w-28 shrink-0 text-white/40">{{ entry.label }}</dt>
							<dd class="text-white/80">{{ entry.value }}</dd>
						</div>
					</dl>

					<div v-else class="surface space-y-2.5 p-4 text-sm">
						<div class="flex gap-3">
							<span class="w-28 shrink-0 text-white/40">Source</span>
							<span class="text-white/80">{{ title.sourceLabel || "Internet Archive" }}</span>
						</div>
						<div v-if="title.creator" class="flex gap-3">
							<span class="w-28 shrink-0 text-white/40">Auteur</span>
							<span class="text-white/80">{{ title.creator }}</span>
						</div>
						<div v-if="title.downloads" class="flex gap-3">
							<span class="w-28 shrink-0 text-white/40">Vues</span>
							<span class="text-white/80">{{ title.downloads.toLocaleString("fr-FR") }}</span>
						</div>
						<div v-if="title.streams?.length" class="flex gap-3">
							<span class="w-28 shrink-0 text-white/40">Fichiers</span>
							<span class="text-white/80">{{ title.streams.length }} qualités lisibles</span>
						</div>
						<div v-if="title.licenseLabel" class="flex gap-3">
							<span class="w-28 shrink-0 text-white/40">Droits</span>
							<a
								v-if="title.license"
								:href="title.license"
								target="_blank"
								rel="noopener noreferrer"
								class="text-emerald-300/90 underline decoration-dotted underline-offset-2 hover:text-emerald-200"
							>
								{{ title.licenseLabel }}
							</a>
							<span v-else class="text-white/80">{{ title.licenseLabel }}</span>
						</div>
					</div>
				</div>
			</section>


			<!-- Distribution -->
			<section v-if="title.credits?.cast?.length" class="mx-auto max-w-[1800px] space-y-3 px-4 pt-10 md:px-6 lg:px-8">
				<h2 class="section-title">Distribution</h2>
				<div class="rail">
					<div
						v-for="person in title.credits.cast"
						:key="person.id"
						class="rail-item w-24 text-center sm:w-28"
					>
						<img
							v-if="person.photo"
							:src="person.photo"
							:alt="person.name"
							loading="lazy"
							class="mx-auto h-24 w-24 rounded-full object-cover ring-1 ring-white/10 sm:h-28 sm:w-28"
						/>
						<div
							v-else
							class="mx-auto grid h-24 w-24 place-items-center rounded-full bg-inkCard text-lg font-bold text-white/35 ring-1 ring-white/10 sm:h-28 sm:w-28"
						>
							{{ person.name.charAt(0) }}
						</div>
						<p class="mt-2 line-clamp-2 text-xs font-medium text-white/85">{{ person.name }}</p>
						<p v-if="person.character" class="line-clamp-2 text-[11px] text-white/40">{{ person.character }}</p>
					</div>
				</div>
			</section>

			<!-- Images du film -->
			<section v-if="title.screenshots?.length" class="mx-auto max-w-[1800px] space-y-3 px-4 pt-10 md:px-6 lg:px-8">
				<h2 class="section-title">Images</h2>
				<div class="rail">
					<img
						v-for="(image, index) in title.screenshots"
						:key="`${image}-${index}`"
						:src="image"
						:alt="`${title.title} — image ${index + 1}`"
						loading="lazy"
						class="rail-item h-32 w-56 rounded-xl object-cover ring-1 ring-white/10 sm:h-40 sm:w-72"
					/>
				</div>
			</section>

			<!-- Episodes d'une serie libre de droits : lecture ou telechargement par episode -->
			<section
				v-if="archiveEpisodes.length"
				id="episodes"
				class="mx-auto max-w-[1800px] scroll-mt-24 space-y-4 px-4 pt-10 md:px-6 lg:px-8"
			>
				<h2 class="section-title">Épisodes ({{ archiveEpisodes.length }})</h2>

				<ul class="space-y-2">
					<li
						v-for="episode in archiveEpisodes"
						:key="episode.label"
						class="surface flex flex-wrap items-center gap-3 p-3 transition-normal hover:border-white/20"
					>
						<span class="badge bg-white/10 text-white/70">{{ episode.label }}</span>

						<div class="min-w-0 flex-1">
							<h3 class="line-clamp-1 text-sm font-semibold text-white/90">
								{{ episode.name || episode.title || episode.label }}
							</h3>
							<p class="text-[11px] text-white/40">{{ episodeMeta(episode) }}</p>
						</div>

						<div class="flex items-center gap-2">
							<RouterLink
								:to="{ ...watchRoute(title), query: { episode: episode.number } }"
								class="btn btn-inverse px-3 py-2 text-xs"
							>
								<AppIcon name="play" :size="14" filled />
								<span>Lire</span>
							</RouterLink>

							<DownloadButton
								:item="title"
								:stream="episode.streams?.[0] || null"
								:episode="episode"
								compact
							/>
						</div>
					</li>
				</ul>
			</section>

			<!-- Saisons et épisodes (séries TMDB) -->
			<section
				v-if="title.tmdbId && title.seasons?.length"
				class="mx-auto max-w-[1800px] space-y-4 px-4 pt-10 md:px-6 lg:px-8"
			>
				<div class="flex items-end justify-between gap-4">
					<h2 class="section-title">Épisodes</h2>
					<span class="text-xs text-white/40">{{ episodes.length }} épisodes affichés</span>
				</div>

				<div class="rail -mx-1 px-1">
					<button
						v-for="season in title.seasons"
						:key="season.seasonNumber"
						type="button"
						class="chip"
						:class="selectedSeason === season.seasonNumber ? 'chip-active' : ''"
						@click="loadEpisodes(season.seasonNumber)"
					>
						{{ season.name }} ({{ season.episodeCount }})
					</button>
				</div>

				<div v-if="isLoadingEpisodes" class="space-y-3">
					<div v-for="index in 4" :key="`ep-${index}`" class="skeleton h-20 w-full"></div>
				</div>

				<ul v-else class="space-y-2">
					<li
						v-for="episode in episodes"
						:key="episode.id"
						class="surface flex gap-3 overflow-hidden p-2.5 transition-normal hover:border-white/20"
					>
						<img
							v-if="episode.still"
							:src="episode.still"
							:alt="episode.name"
							loading="lazy"
							class="h-16 w-28 shrink-0 rounded-lg object-cover"
						/>
						<div class="min-w-0 flex-1 space-y-1">
							<div class="flex items-baseline gap-2">
								<span class="text-xs font-bold text-brandSoft">E{{ episode.episodeNumber }}</span>
								<h3 class="line-clamp-1 text-sm font-semibold text-white/90">{{ episode.name }}</h3>
							</div>
							<p class="line-clamp-2 text-xs text-white/45">
								{{ episode.overview || "Pas de résumé pour cet épisode." }}
							</p>
							<p class="text-[11px] text-white/35">
								{{
									[
										formatDate(episode.airDate),
										formatRuntime(episode.runtime),
										episode.rating ? `★ ${episode.rating}` : null,
									]
										.filter(Boolean)
										.join(" • ")
								}}
							</p>
						</div>
						<RouterLink
							:to="watchRoute(title)"
							class="icon-btn h-9 w-9 self-center"
							:aria-label="`Où regarder l'épisode ${episode.episodeNumber}`"
						>
							<AppIcon name="play" :size="15" filled />
						</RouterLink>
					</li>
				</ul>
			</section>


			<!-- Où regarder légalement -->
			<section v-if="title.mediaType !== 'archive'" class="mx-auto max-w-[1800px] space-y-4 px-4 pt-10 md:px-6 lg:px-8">
				<h2 class="section-title">Où regarder gratuitement (légalement)</h2>

				<div v-if="freeProviders.length" class="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
					<div v-for="group in freeProviders" :key="group.label" class="surface p-4">
						<p class="badge badge-free mb-3">{{ group.label }}</p>
						<ul class="space-y-2">
							<li v-for="provider in group.items" :key="provider.id" class="flex items-center gap-3">
								<img
									v-if="provider.logo"
									:src="provider.logo"
									:alt="provider.name"
									class="h-8 w-8 rounded-md object-cover"
								/>
								<span class="text-sm text-white/85">{{ provider.name }}</span>
							</li>
						</ul>
					</div>
				</div>

				<ul class="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-3">
					<li v-for="link in legalLinks" :key="link.id">
						<a
							v-if="!link.internal"
							:href="link.url"
							target="_blank"
							rel="noopener noreferrer"
							class="surface flex h-full items-center gap-3 p-3.5 transition-normal hover:border-white/20"
						>
							<span class="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-white/5 text-white/70">
								<AppIcon name="link" :size="16" />
							</span>
							<span class="min-w-0">
								<span class="flex items-center gap-2 text-sm font-semibold text-white/90">
									{{ link.name }}
									<span v-if="link.free" class="badge badge-free">Gratuit</span>
								</span>
								<span class="line-clamp-2 block text-[11px] text-white/40">{{ link.description }}</span>
							</span>
							<AppIcon name="external" :size="15" class="ml-auto text-white/30" />
						</a>

						<RouterLink
							v-else
							:to="link.url"
							class="surface flex h-full items-center gap-3 p-3.5 transition-normal hover:border-white/20"
						>
							<span class="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-emerald-500/15 text-emerald-400">
								<AppIcon name="gift" :size="16" />
							</span>
							<span class="min-w-0">
								<span class="flex items-center gap-2 text-sm font-semibold text-white/90">
									{{ link.name }}
									<span class="badge badge-free">Gratuit</span>
								</span>
								<span class="line-clamp-2 block text-[11px] text-white/40">{{ link.description }}</span>
							</span>
							<AppIcon name="chevronRight" :size="15" class="ml-auto text-white/30" />
						</RouterLink>
					</li>
				</ul>

				<div v-if="paidProviders.length" class="grid gap-3 pt-1 sm:grid-cols-2 lg:grid-cols-3">
					<div v-for="group in paidProviders" :key="group.label" class="surface-soft p-3.5">
						<p class="mb-2 text-[11px] font-semibold uppercase tracking-wide text-white/40">{{ group.label }}</p>
						<div class="flex flex-wrap items-center gap-2">
							<img
								v-for="provider in group.items"
								:key="provider.id"
								:src="provider.logo"
								:alt="provider.name"
								class="h-7 w-7 rounded-md object-cover ring-1 ring-white/10"
							/>
						</div>
					</div>
				</div>

				<a
					v-if="title.providers?.link"
					:href="title.providers.link"
					target="_blank"
					rel="noopener noreferrer"
					class="inline-flex items-center gap-2 text-xs font-medium text-brandSoft hover:underline"
				>
					Voir toutes les offres légales
					<AppIcon name="external" :size="13" />
				</a>

			</section>

			<!-- Source et licence (films libres) -->
			<section v-if="externalLinks.length" class="mx-auto max-w-[1800px] space-y-3 px-4 pt-10 md:px-6 lg:px-8">
				<h2 class="section-title">Source &amp; licence</h2>
				<ul class="grid gap-2.5 sm:grid-cols-2">
					<li v-for="link in externalLinks" :key="link.id">
						<a
							:href="link.url"
							target="_blank"
							rel="noopener noreferrer"
							class="surface flex items-center gap-3 p-3.5 transition-normal hover:border-white/20"
						>
							<span class="grid h-9 w-9 place-items-center rounded-lg bg-white/5 text-white/70">
								<AppIcon name="archive" :size="16" />
							</span>
							<span>
								<span class="block text-sm font-semibold text-white/90">{{ link.name }}</span>
								<span class="block text-[11px] text-white/40">{{ link.description }}</span>
							</span>
							<AppIcon name="external" :size="15" class="ml-auto text-white/30" />
						</a>
					</li>
				</ul>
			</section>

			<!-- Suggestions -->
			<section
				v-if="title.similar?.length || title.related?.length"
				class="mx-auto max-w-[1800px] space-y-4 px-4 pt-12 md:px-6 lg:px-8"
			>
				<h2 class="section-title">Vous aimerez aussi</h2>
				<MediaGrid :items="title.similar?.length ? title.similar : title.related" />
			</section>
		</template>
	</div>
</template>
