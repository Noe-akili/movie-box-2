<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import {
	downloadStreamUrl,
	fetchDownload,
	fetchInternalStreams,
	fetchTitle,
	fetchTransfer,
	mediaUrl,
	proxyStreamUrl,
	receivedTransferFileUrl,
} from "../api/client";
import AppIcon from "../components/AppIcon.vue";
import DownloadButton from "../components/DownloadButton.vue";
import EmptyState from "../components/EmptyState.vue";
import MediaRow from "../components/MediaRow.vue";
import VideoPlayer from "../components/VideoPlayer.vue";
import { useDownloads } from "../stores/downloads";
import { useLibrary } from "../stores/library";
import { toast } from "../stores/toast";
import { formatCount, formatSize } from "../util/format";
import { isFree, isSeriesTitle, mediaTypeLabel } from "../util/media";

const props = defineProps({
	mediaType: { type: String, required: true },
	id: { type: String, required: true },
});

const router = useRouter();
const route = useRoute();
const { addToHistory, isFavorite, toggleFavorite } = useLibrary();

const title = ref(null);
const isLoading = ref(true);
const errorMessage = ref(null);
const selectedStream = ref(0);
const selectedEpisode = ref(0);
const useEmbed = ref(false);

// Une archive peut contenir plusieurs episodes : on bascule de l'un a l'autre.
const episodes = computed(() => (Array.isArray(title.value?.episodes) ? title.value.episodes : []));
// Une série (kind/`subjectType`) qui fournit ses épisodes se lit épisode par
// épisode ; un titre sans liste d'épisodes retombe sur ses flux directs.
const isSeries = computed(() => isSeriesTitle(title.value) && episodes.value.length > 0);
const currentEpisode = computed(() => (isSeries.value ? episodes.value[selectedEpisode.value] || null : null));
const streams = computed(() => {
	if (isSeries.value) return currentEpisode.value?.streams || [];
	return title.value?.streams || [];
});

const currentStream = computed(() => streams.value[selectedStream.value] || null);

// Lecture en ligne : si le flux direct échoue (CDN anti-hotlink, WebView), on
// rejoue via le proxy local du backend ; si un flux proxifié échoue, on tente
// l'URL CDN directe. On garde la dernière erreur pour prévenir l'utilisateur.
const playbackMode = ref("default");
const playbackError = ref(null);

const playerSource = computed(() => {
	const stream = currentStream.value;
	if (!stream?.url) return "";
	const raw = stream.url;

	if (playbackMode.value === "proxy" && !raw.startsWith("/")) {
		return mediaUrl(proxyStreamUrl(raw));
	}
	if (playbackMode.value === "direct" && raw.startsWith("/") && stream.directUrl) {
		return mediaUrl(stream.directUrl);
	}
	return mediaUrl(raw);
});

const onPlayerError = () => {
	const stream = currentStream.value;
	if (!stream?.url) return;

	if (isPartialLocal.value) {
		playbackError.value =
			"Ce fichier n'est pas encore assez téléchargé pour être lu. Réessaie dans un instant, le transfert continue.";
		toast.error("Lecture partielle impossible pour l'instant");
		return;
	}

	if (playbackMode.value === "default") {
		// Flux proxifié : on tente le CDN en direct. Flux direct : on tente le proxy.
		if (stream.url.startsWith("/") && stream.directUrl) {
			playbackMode.value = "direct";
			return;
		}
		if (!stream.url.startsWith("/")) {
			playbackMode.value = "proxy";
			return;
		}
	}

	playbackError.value =
		"La vidéo n'a pas pu être lue en ligne. Essaie une autre qualité ou télécharge-la pour la regarder hors ligne.";
	toast.error("Lecture en ligne impossible");
};

const retryPlayback = () => {
	playbackMode.value = "default";
	playbackError.value = null;
};

watch([selectedStream, selectedEpisode], () => {
	playbackMode.value = "default";
	playbackError.value = null;
});

// Qualités proposées dans le menu de réglages du lecteur (masqué sous 2 entrées).
const playerQualities = computed(() =>
	streams.value.map((stream) => ({
		html: `${stream.quality || "Auto"} · ${stream.format || "vidéo"}`,
		url: mediaUrl(stream.url),
	})),
);
const free = computed(() => isFree(title.value));
const trailer = computed(() => title.value?.trailer || null);
const legalLinks = computed(() => title.value?.legalLinks || []);
const related = computed(() => title.value?.related || title.value?.similar || []);
const saved = computed(() => (title.value ? isFavorite(title.value) : false));
const isLocal = computed(() =>
	["local", "received"].includes(props.mediaType),
);
// Identifiant stable par vidéo : sert de clé pour reprendre la lecture plus tard.
const playerId = computed(() => {
	if (props.mediaType === "received") return `received:${props.id}`;
	if (isLocal.value) return `local:${props.id}`;
	const base = title.value?.archiveId || title.value?.internalId || title.value?.id || props.id;
	const episodePart =
		isSeries.value && currentEpisode.value
			? `:s${currentEpisode.value.season ?? 0}e${currentEpisode.value.number ?? 0}`
			: "";
	return `${base}${episodePart}`;
});
const { items: localDownloads } = useDownloads();
// Entree brute du fichier local en cours (permet de trouver l'episode suivant).
const localEntry = ref(null);
const isPartialLocal = computed(
	() => isLocal.value && Boolean(localEntry.value) && localEntry.value.status !== "completed",
);

const isSameSeries = (entry, target) =>
	entry.kind === "episode" && target?.kind === "episode" && entry.seriesTitle === target.seriesTitle;

/**
 * Épisode suivant de la même série parmi les fichiers déjà téléchargés :
 * ordre naturel (saison, numéro), avec retour au début en fin de série.
 */
const nextLocalEpisode = computed(() => {
	if (!isLocal.value || !localEntry.value?.seriesTitle) return null;

	const siblings = localDownloads.value
		.filter((entry) => isSameSeries(entry, localEntry.value) && entry.status === "completed")
		.sort(
			(a, b) =>
				(a.seasonNumber || 0) - (b.seasonNumber || 0) ||
				(a.episodeNumber || 0) - (b.episodeNumber || 0),
		);

	if (siblings.length < 2) return null;
	const index = siblings.findIndex((entry) => entry.id === localEntry.value.id);
	if (index === -1) return siblings[0];
	return siblings[(index + 1) % siblings.length];
});

/** Épisode suivant de la série affichée (retour au premier en fin de liste). */
const nextEpisodeIndex = computed(() => {
	if (!isSeries.value || episodes.value.length < 2) return -1;
	return (selectedEpisode.value + 1) % episodes.value.length;
});
const nextSeriesEpisode = computed(() => episodes.value[nextEpisodeIndex.value] || null);
const episodeTitle = (episode) =>
	episode?.name ||
	episode?.title ||
	episode?.label ||
	(episode?.number ? `Épisode ${episode.number}` : "Épisode");

const relatedHref = computed(() => ({
	path: "/free",
	query: { q: title.value?.title || "" },
}));

/** Fiche minimale reconstruite pour un fichier deja telecharge (lecture sans Internet Archive). */
const buildLocalTitle = (entry) => ({
	id: entry.id,
	mediaType: "archive",
	archiveId: entry.sourceId,
	title: entry.kind === "episode" ? entry.seriesTitle || entry.title : entry.title,
	overview: "",
	poster: entry.poster || null,
	backdrop: entry.backdrop || null,
	streams: [
		{
			name: entry.fileName,
			quality: entry.quality || "HD",
			format: entry.format || "Vidéo",
			size: entry.bytesTotal,
			url: downloadStreamUrl(entry.id),
		},
	],
	episodes: [],
	kind: entry.kind,
	playable: true,
	sourceLabel: "Fichier téléchargé — lecture hors ligne",
	related: [],
	legalLinks: [],
	externalLinks: [],
});

/** Fiche minimale pour un fichier reçu via la salle Wi-Fi locale. */
const buildReceivedTitle = (file) => {
	const extension = String(file.name || "").split(".").pop() || "Vidéo";
	return {
		id: file.id,
		mediaType: "archive",
		archiveId: `received:${file.id}`,
		title: String(file.name || "Fichier reçu").replace(/\.[^.]+$/, ""),
		overview: "",
		poster: null,
		backdrop: null,
		streams: [
			{
				name: file.name,
				quality: "Reçu",
				format: extension.toUpperCase(),
				size: file.size,
				url: receivedTransferFileUrl(file.id),
			},
		],
		episodes: [],
		kind: "movie",
		playable: true,
		sourceLabel: "Fichier reçu — lecture hors ligne",
		related: [],
		legalLinks: [],
		externalLinks: [],
	};
};

const selectEpisode = async (index) => {
	selectedEpisode.value = index;
	selectedStream.value = 0;
	useEmbed.value = false;

	const episode = episodes.value[index];
	if (
		props.mediaType === "internal" &&
		episode &&
		!(episode.streams && episode.streams.length) &&
		title.value?.internalId
	) {
		try {
			const data = await fetchInternalStreams(title.value.internalId, {
				season: episode.season ?? 0,
				episode: episode.number ?? 1,
			});
			episode.streams = data?.streams || [];
		} catch (error) {
			toast.error(error.message);
		}
	}
};

const selectEpisodeFromQuery = () => {
	const wanted = Number(route.query.episode);
	if (!Number.isFinite(wanted)) return;

	const rawSeason = route.query.season;
	const hasSeason = rawSeason !== undefined && rawSeason !== "" && Number.isFinite(Number(rawSeason));
	const wantedSeason = Number(rawSeason);

	const index = episodes.value.findIndex((episode) => {
		if (episode.number !== wanted) return false;
		if (!hasSeason) return true;
		return Number(episode.season) === wantedSeason;
	});
	const hasStreams = Boolean(episodes.value[index]?.streams?.length);
	if (index >= 0 && (index !== selectedEpisode.value || !hasStreams)) selectEpisode(index);
};

/** Passe à l'épisode suivant et lance la lecture immédiatement. */
const playNextEpisode = () => {
	if (nextEpisodeIndex.value >= 0) selectEpisode(nextEpisodeIndex.value);
};

/** Depuis la liste sous le titre : lance l'épisode et remonte sur le lecteur. */
const playEpisode = async (index) => {
	await selectEpisode(index);
	document.getElementById("watch-player")?.scrollIntoView({ block: "start" });
};

const load = async () => {
	isLoading.value = true;
	errorMessage.value = null;
	title.value = null;
	localEntry.value = null;
	selectedStream.value = 0;
	selectedEpisode.value = 0;
	useEmbed.value = false;

	try {
		if (props.mediaType === "received") {
			const data = await fetchTransfer();
			const file = (data?.received || []).find((item) => item.id === props.id);
			if (!file) throw new Error("Ce fichier reçu n'est plus disponible");
			title.value = buildReceivedTitle(file);
			return;
		}

		if (isLocal.value) {
			const entry = await fetchDownload(props.id);
			localEntry.value = entry;
			title.value = buildLocalTitle(entry);
			return;
		}

		const data = await fetchTitle(props.mediaType, props.id);
		title.value = data;
		selectEpisodeFromQuery();
		addToHistory(data);
	} catch (error) {
		errorMessage.value = error.message;
	} finally {
		isLoading.value = false;
	}
};

const goBack = () => {
	if (window.history.length > 1) router.back();
	else router.push({ name: "home" });
};

const onToggleFavorite = () => {
	if (!title.value) return;
	const added = toggleFavorite(title.value);
	if (added) toast.success(`« ${title.value.title} » ajouté à Ma liste`);
	else toast.info(`« ${title.value.title} » retiré de Ma liste`);
};

onMounted(load);
watch(() => [props.mediaType, props.id], load);
watch(() => [route.query.season, route.query.episode], selectEpisodeFromQuery);
</script>

<template>
	<div class="min-h-screen bg-ink pb-nav">
		<header class="header-safe sticky top-0 z-30 flex items-center gap-3 border-b border-white/5 bg-ink/95 px-4 py-3 backdrop-blur-xl md:px-6">
			<button type="button" class="icon-btn h-9 w-9" aria-label="Retour" @click="goBack">
				<AppIcon name="arrowLeft" :size="17" />
			</button>
			<div class="min-w-0 flex-1">
				<h1 class="line-clamp-1 text-sm font-semibold text-white">{{ title?.title || "Lecture" }}</h1>
				<p v-if="title" class="text-[11px] text-white/40">
					{{
						isLocal
							? isPartialLocal
								? `Lecture partielle — ${formatSize(localEntry.bytesReceived)} / ${formatSize(localEntry.bytesTotal)}`
								: "Lecture hors ligne"
							: free
								? "Lecture gratuite — domaine public / libre"
								: "Bande-annonce officielle et sources légales"
					}}
				</p>
			</div>
			<button
				v-if="title"
				type="button"
				class="icon-btn h-9 w-9"
				:aria-label="saved ? 'Retirer de Ma liste' : 'Ajouter à Ma liste'"
				@click="onToggleFavorite"
			>
				<AppIcon name="heart" :size="17" :filled="saved" />
			</button>
		</header>

		<div v-if="isLoading" class="space-y-4">
			<div class="skeleton aspect-video w-full rounded-none"></div>
			<div class="space-y-3 px-4 md:px-6">
				<div class="skeleton h-5 w-1/3"></div>
				<div class="skeleton h-3 w-2/3"></div>
			</div>
		</div>

		<EmptyState
			v-else-if="errorMessage"
			icon="info"
			title="Lecture impossible"
			:description="errorMessage"
			action-label="Réessayer"
			@action="load"
		/>

		<template v-else-if="title">
			<div id="watch-player" class="w-full scroll-mt-20 bg-black">
				<div class="relative mx-auto aspect-video w-full max-w-[1800px]">
					<VideoPlayer
						v-if="free && currentStream && !useEmbed"
						:key="`${playerId}:${playerSource}:${playbackMode}`"
						:id="playerId"
						:src="playerSource"
						:poster="title.backdrop || title.poster"
						:qualities="playerQualities"
						autoplay
						class="h-full w-full bg-black"
						@error="onPlayerError"
					/>

					<iframe
						v-else-if="free && useEmbed"
						:src="title.embedUrl"
						title="Lecteur Internet Archive"
						allow="autoplay; fullscreen; encrypted-media"
						allowfullscreen
						class="h-full w-full border-0 bg-black"
					></iframe>

					<iframe
						v-else-if="trailer"
						:src="trailer.embedUrl"
						:title="trailer.name"
						allow="autoplay; accelerometer; encrypted-media; picture-in-picture"
						allowfullscreen
						class="h-full w-full border-0 bg-black"
					></iframe>

					<div v-else class="grid h-full w-full place-items-center bg-inkCard px-6 text-center">
						<div class="max-w-md space-y-2">
							<AppIcon name="info" :size="26" class="mx-auto text-white/40" />
							<p class="text-sm font-semibold text-white">Pas de bande-annonce, mais le film reste accessible</p>
							<p class="text-xs text-white/45">
								Cette œuvre est protégée par le droit d'auteur : Movie Box te renvoie vers les plateformes
								officielles, dont les offres gratuites financées par la publicité.
							</p>
						</div>
					</div>

					<div
						v-if="isPartialLocal"
						class="pointer-events-none absolute inset-x-0 top-0 z-20 bg-black/70 px-4 py-2 text-center text-[11px] text-white/75 backdrop-blur"
					>
						Lecture partielle : {{ formatSize(localEntry.bytesReceived) }} /
						{{ formatSize(localEntry.bytesTotal) }} — le téléchargement continue en arrière-plan
					</div>

					<!-- Épisode suivant : icône seule, en bas de la vidéo. -->
					<RouterLink
						v-if="nextLocalEpisode"
						:to="{ name: 'watch-local', params: { id: nextLocalEpisode.id } }"
						class="absolute bottom-20 right-4 z-30 grid h-11 w-11 place-items-center rounded-full border border-white/20 bg-black/75 text-white shadow-lg transition-normal hover:bg-black/90"
						:aria-label="`Épisode suivant : ${nextLocalEpisode.episodeName || nextLocalEpisode.title}`"
						:title="`Épisode suivant : ${nextLocalEpisode.episodeName || nextLocalEpisode.title}`"
					>
						<AppIcon name="chevronRight" :size="22" />
					</RouterLink>

					<button
						v-else-if="nextSeriesEpisode"
						type="button"
						class="absolute bottom-20 right-4 z-30 grid h-11 w-11 place-items-center rounded-full border border-white/20 bg-black/75 text-white shadow-lg transition-normal hover:bg-black/90"
						:aria-label="`Épisode suivant : ${episodeTitle(nextSeriesEpisode)}`"
						:title="`Épisode suivant : ${episodeTitle(nextSeriesEpisode)}`"
						@click="playNextEpisode"
					>
						<AppIcon name="chevronRight" :size="22" />
					</button>

					<div
						v-if="playbackError"
						class="absolute inset-x-0 bottom-0 z-20 flex flex-wrap items-center justify-between gap-2 bg-black/85 px-4 py-3 text-xs text-white/80 backdrop-blur"
					>
						<span class="min-w-0 flex-1">{{ playbackError }}</span>
						<button type="button" class="btn btn-inverse shrink-0 px-3 py-1.5 text-xs" @click="retryPlayback">
							Réessayer
						</button>
					</div>
				</div>
			</div>

			<section v-if="free && streams.length" class="mx-auto max-w-[1800px] space-y-3 px-4 pt-5 md:px-6 lg:px-8">
				<div class="flex flex-wrap items-center gap-2">
					<h2 class="section-title">Qualité &amp; sources</h2>
					<DownloadButton
						v-if="currentStream && !isLocal"
						:item="title"
						:stream="currentStream"
						:episode="currentEpisode"
						:label="isSeries ? 'Télécharger cet épisode' : 'Télécharger'"
					/>
					<button v-if="title.embedUrl" type="button" class="chip !py-1 text-xs" @click="useEmbed = !useEmbed">
						{{ useEmbed ? "Utiliser le lecteur intégré" : "Utiliser le lecteur Internet Archive" }}
					</button>
				</div>

				<div class="rail -mx-1 px-1">
					<button
						v-for="(stream, index) in streams"
						:key="stream.url"
						type="button"
						class="chip"
						:class="selectedStream === index && !useEmbed ? 'chip-active' : ''"
						@click="((selectedStream = index), (useEmbed = false))"
					>
						{{ stream.quality }} · {{ stream.format }}<span v-if="stream.size"> · {{ formatSize(stream.size) }}</span>
					</button>
				</div>

				<p v-if="!isLocal" class="text-[11px] text-white/35">
					{{ title.sourceLabel }}
					<span v-if="formatCount(title.downloads)"> — {{ formatCount(title.downloads) }} vues sur Internet Archive</span>
				</p>
			</section>

			<section class="mx-auto max-w-[1800px] space-y-4 px-4 pt-6 md:px-6 lg:px-8">
				<div class="grid gap-6 lg:grid-cols-[minmax(0,2fr)_minmax(0,1fr)]">
					<div class="space-y-2">
						<h2 class="section-title">{{ title.title }}</h2>
						<p class="text-xs text-white/40">
							{{
								[
									title.year,
									isSeriesTitle(title) ? "Série" : free ? "Film libre de droits" : mediaTypeLabel(title),
								]
									.filter(Boolean)
									.join(" • ")
							}}
						</p>

						<!-- Autres épisodes de la série, juste sous le nom. Un clic lance
						     immédiatement la lecture de l'épisode choisi. -->
						<div v-if="isSeries" class="space-y-2 pt-2">
							<div class="flex items-center justify-between gap-3">
								<h3 class="text-sm font-semibold text-white/85">Épisodes</h3>
								<span class="text-[11px] text-white/40">{{ episodes.length }} épisodes</span>
							</div>
							<div class="rail -mx-1 px-1">
								<button
									v-for="(episode, index) in episodes"
									:key="`${episode.season}-${episode.number}`"
									type="button"
									class="rail-item surface w-44 p-2.5 text-left transition-normal hover:border-white/25"
									:class="selectedEpisode === index ? 'border-brand/60 bg-brand/10' : ''"
									:title="`Lire ${episode.label} — ${episodeTitle(episode)}`"
									@click="playEpisode(index)"
								>
									<span class="block text-[11px] font-semibold text-brandSoft">{{ episode.label }}</span>
									<span class="mt-0.5 line-clamp-2 block text-xs font-medium text-white/90">
										{{ episodeTitle(episode) }}
									</span>
									<span class="mt-1 flex items-center gap-1 text-[10px] text-white/40">
										<AppIcon name="play" :size="10" filled />
										{{ selectedEpisode === index ? "En cours" : "Lire" }}
									</span>
								</button>
							</div>
						</div>

						<p v-if="title.overview" class="text-sm leading-relaxed text-white/70">
							{{ title.overview }}
						</p>
					</div>

					<div v-if="!isLocal" class="surface space-y-2.5 p-4 text-sm">
						<div v-if="title.creator" class="flex gap-3">
							<span class="w-24 shrink-0 text-white/40">Auteur</span>
							<span class="text-white/80">{{ title.creator }}</span>
						</div>
						<div class="flex gap-3">
							<span class="w-24 shrink-0 text-white/40">Source</span>
							<span class="text-white/80">{{ title.sourceLabel || "TMDB" }}</span>
						</div>
						<a
							v-if="title.detailsUrl"
							:href="title.detailsUrl"
							target="_blank"
							rel="noopener noreferrer"
							class="inline-flex items-center gap-2 text-xs font-medium text-brandSoft hover:underline"
						>
							Fiche Internet Archive
							<AppIcon name="external" :size="13" />
						</a>
					</div>
				</div>

				<ul v-if="!free" class="grid gap-2.5 sm:grid-cols-2 lg:grid-cols-3">
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
			</section>

			<section v-if="related.length" class="mx-auto max-w-[1800px] pt-8">
				<MediaRow
					:title="free ? 'Autres films gratuits' : 'Dans le même esprit'"
					:items="related"
					:href="free ? relatedHref : null"
				/>
			</section>
		</template>
	</div>
</template>
