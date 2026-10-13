<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { fetchInternalStreams } from "../api/client";
import { useDownloads } from "../stores/downloads";
import { toast } from "../stores/toast";
import {
	downloadFileName,
	downloadSourceId,
	isInternalSource,
	pickQualityStream,
	qualityHeight,
} from "../util/downloads";
import { formatSize } from "../util/format";
import AppIcon from "./AppIcon.vue";

/**
 * Assistant de telechargement : qualite -> saison (series) -> episodes -> selection multiple.
 * Les fichiers sont stockes par le backend Movie Box (lecture hors ligne).
 */
const props = defineProps({
	item: { type: Object, required: true },
	stream: { type: Object, default: null },
	episode: { type: Object, default: null },
	label: { type: String, default: "Télécharger" },
});

const emit = defineEmits(["close"]);

const { start } = useDownloads();

const isInternal = computed(() => isInternalSource(props.item));
const internalId = computed(
	() =>
		props.item?.internalId ||
		String(props.item?.archiveId || props.item?.sourceId || "").replace(/^internal:/, ""),
);
const itemTitle = computed(() => props.item?.title || "Vidéo");

// `episodes` est un tableau sur une fiche complète, mais un simple compte sur les
// cartes du catalogue : on ne garde que le cas exploitable par l'assistant.
const episodes = computed(() => (Array.isArray(props.item?.episodes) ? props.item.episodes : []));

// Type réel du titre, déduit des données de l'API (kind, épisodes, mediaType) :
// une série gratuite ne doit jamais être prise pour un film.
const isSeries = computed(
	() =>
		episodes.value.length > 0 ||
		props.item?.kind === "series" ||
		props.item?.mediaType === "tv" ||
		Boolean(props.episode),
);

// Assistant saison/épisodes seulement si des épisodes sont réellement listés :
// sinon on retombe sur le téléchargement direct du flux du titre (cas d'une série
// dont l'API n'a pas encore renvoyé sa liste d'épisodes).
const effectiveSeries = computed(() => isSeries.value && seasons.value.length > 0);

const seasons = computed(() => {
	const list = [];
	for (const episode of episodes.value) {
		const season = episode.season ?? 1;
		if (!list.includes(season)) list.push(season);
	}
	return list.sort((a, b) => a - b);
});

const step = ref("choose");
const selectedQuality = ref(null);
const selectedSeason = ref(null);
const selectedKeys = ref([]);
const qualityOptions = ref([]);
const isLoadingQuality = ref(false);
const isWorking = ref(false);

const keyOf = (episode) => `${episode.season ?? 1}:${episode.number ?? 1}`;

const movieStream = computed(() => {
	const list = props.item?.streams || [];
	return pickQualityStream(list, selectedQuality.value) || props.stream || null;
});

const seasonEpisodes = computed(() =>
	episodes.value
		.filter((episode) => (episode.season ?? 1) === selectedSeason.value)
		.map((episode) => {
			const list = episode.streams || [];
			// Qualité exacte si disponible, sinon la plus proche : certains épisodes
			// (saisons récentes) n'existent qu'en 480p.
			const stream = pickQualityStream(list, selectedQuality.value);
			return {
				episode,
				stream,
				// Source interne : l'API fournit les flux épisode par épisode, on les
				// résout au moment du téléchargement.
				available: isInternal.value ? true : Boolean(stream),
			};
		}),
);

const availableEpisodes = computed(() => seasonEpisodes.value.filter((entry) => entry.available));
const selectedCount = computed(() => selectedKeys.value.length);
const allSelected = computed(
	() => availableEpisodes.value.length > 0 && selectedCount.value === availableEpisodes.value.length,
);

/**
 * Qualites reellement proposees. L'API MovieBox ne fournit pas les memes qualites
 * pour toutes les saisons (la saison 8 de The Rookie n'existe qu'en 480p) : on
 * interroge donc un episode de la saison affichee plutot que de se fier au seul
 * premier episode de la fiche.
 */
const loadQualities = async (season = null) => {
	isLoadingQuality.value = true;

	try {
		const list = [];
		const push = (quality) => {
			const value = String(quality || "").trim();
			if (value && !list.includes(value)) list.push(value);
		};

		for (const stream of props.item?.streams || []) push(stream.quality);
		if (props.stream?.quality) push(props.stream.quality);

		if (isSeries.value) {
			const pool = season
				? episodes.value.filter((episode) => (episode.season ?? 1) === season)
				: episodes.value;
			for (const episode of pool) for (const stream of episode.streams || []) push(stream.quality);

			// Les episodes arrivent sans flux : on demande ceux d'un episode temoin.
			if (isInternal.value && internalId.value) {
				const sample = pool[0] || episodes.value[0];
				if (sample) {
					try {
						const data = await fetchInternalStreams(internalId.value, {
							season: sample.season ?? 1,
							episode: sample.number ?? 1,
						});
						for (const stream of data?.streams || []) push(stream.quality);
					} catch {
						// API indisponible pour cet episode : on garde les qualites connues.
					}
				}
			}
		}

		if (!list.length) list.push("Auto");
		list.sort((a, b) => (qualityHeight(a) ?? 0) - (qualityHeight(b) ?? 0));

		const previous = selectedQuality.value;
		qualityOptions.value = list;
		// On conserve le choix de l'utilisateur s'il existe encore dans cette saison.
		selectedQuality.value =
			previous && list.includes(previous) ? previous : list[list.length - 1];
	} catch {
		qualityOptions.value = props.stream?.quality ? [props.stream.quality] : ["Auto"];
		selectedQuality.value = qualityOptions.value[0];
	} finally {
		isLoadingQuality.value = false;
	}
};

const selectQuality = (quality) => {
	if (selectedQuality.value === quality) return;
	selectedQuality.value = quality;
	selectedKeys.value = [];
};

const toggleEpisode = (entry) => {
	if (!entry.available) return;
	const key = keyOf(entry.episode);
	selectedKeys.value = selectedKeys.value.includes(key)
		? selectedKeys.value.filter((item) => item !== key)
		: [...selectedKeys.value, key];
};

const toggleAll = () => {
	selectedKeys.value = allSelected.value ? [] : availableEpisodes.value.map((entry) => keyOf(entry.episode));
};

const goToEpisodes = () => {
	step.value = "episodes";
};

const goBack = () => {
	step.value = "choose";
};

const close = () => emit("close");

const resolveInternalStream = async (episode) => {
	try {
		const data = await fetchInternalStreams(internalId.value, {
			season: episode.season ?? 1,
			episode: episode.number ?? 1,
		});
		const list = data?.streams || [];
		return pickQualityStream(list, selectedQuality.value);
	} catch {
		return null;
	}
};

const downloadMovie = async () => {
	const stream = movieStream.value;
	const quality = selectedQuality.value;
	const fileName = downloadFileName(props.item, stream, { quality });

	if (!fileName) {
		toast.error("Aucun fichier téléchargeable pour ce titre");
		return;
	}

	await start({
		sourceId: downloadSourceId(props.item),
		fileName,
		title: itemTitle.value,
		kind: "movie",
		poster: props.item?.poster,
		backdrop: props.item?.backdrop,
		quality,
		format: stream?.format,
	});
};

const downloadEpisodes = async () => {
	const chosen = seasonEpisodes.value.filter(
		(entry) => entry.available && selectedKeys.value.includes(keyOf(entry.episode)),
	);

	if (!chosen.length) {
		toast.info("Sélectionne au moins un épisode");
		return;
	}

	for (const entry of chosen) {
		const { episode } = entry;
		const season = episode.season ?? 1;
		const number = episode.number ?? 1;
		const quality = selectedQuality.value;
		let stream = entry.stream;

		// Source interne : chaque épisode a ses propres flux (URL signées qui
		// expirent) — on les résout au moment du téléchargement.
		if (isInternal.value) {
			const resolved = await resolveInternalStream(episode);
			if (resolved) stream = resolved;
		}

		const fileName = downloadFileName(props.item, stream, { episode, quality });

		if (!fileName) continue;

		await start({
			sourceId: downloadSourceId(props.item, episode),
			fileName,
			title: episode.name || episode.title || `Épisode ${number}`,
			kind: "episode",
			seriesTitle: itemTitle.value,
			seasonNumber: season,
			episodeNumber: number,
			episodeName: episode.name || episode.title || null,
			label: episode.label || null,
			poster: props.item?.poster,
			backdrop: props.item?.backdrop,
			quality,
			format: stream?.format,
		});
	}
};

const onPrimary = async () => {
	if (isWorking.value) return;

	// Séries : qualité + saison, puis « Avancer » vers la sélection des épisodes.
	if (effectiveSeries.value && step.value !== "episodes") {
		goToEpisodes();
		return;
	}

	isWorking.value = true;

	try {
		if (effectiveSeries.value) {
			await downloadEpisodes();
			toast.success(
				`${selectedCount.value} épisode${selectedCount.value > 1 ? "s" : ""} en téléchargement — ${itemTitle.value}`,
			);
		} else {
			await downloadMovie();
			toast.success(`Téléchargement lancé — ${itemTitle.value}`);
		}
		close();
	} catch (error) {
		toast.error(error.message);
	} finally {
		isWorking.value = false;
	}
};

const primaryLabel = computed(() => {
	if (effectiveSeries.value && step.value !== "episodes") return "Avancer";
	if (effectiveSeries.value) return `Télécharger (${selectedCount.value})`;
	return props.label;
});

const onKeydown = (event) => {
	if (event.key === "Escape") close();
};

// Changer de saison change les qualités disponibles : on les relit pour la
// saison affichée (une saison récente peut n'exister qu'en 480p).
watch(selectedSeason, (season) => {
	if (effectiveSeries.value && season) loadQualities(season);
});

onMounted(async () => {
	document.body.style.overflow = "hidden";
	window.addEventListener("keydown", onKeydown);

	if (effectiveSeries.value) {
		selectedSeason.value = props.episode?.season ?? seasons.value[0] ?? 1;
		if (props.episode) selectedKeys.value = [keyOf(props.episode)];
	}

	await loadQualities(selectedSeason.value);
});

onBeforeUnmount(() => {
	document.body.style.overflow = "";
	window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
	<Teleport to="body">
		<div class="fixed inset-0 z-[60] flex items-end justify-center sm:items-center">
			<button
				type="button"
				class="absolute inset-0 bg-black/70 backdrop-blur-sm"
				aria-label="Fermer"
				@click="close"
			></button>

			<div
				class="relative flex max-h-[88vh] w-full flex-col overflow-hidden rounded-t-3xl border border-inkLine bg-inkCard shadow-deck sm:max-w-lg sm:rounded-3xl"
				role="dialog"
				aria-modal="true"
			>
				<header class="flex items-center gap-3 border-b border-white/5 px-4 py-3.5">
					<span class="grid h-9 w-9 shrink-0 place-items-center rounded-xl bg-brand/15 text-brandSoft">
						<AppIcon name="download" :size="18" />
					</span>
					<div class="min-w-0 flex-1">
						<h2 class="line-clamp-1 text-sm font-bold text-white">Télécharger</h2>
						<p class="line-clamp-1 text-[11px] text-white/45">{{ itemTitle }}</p>
					</div>
					<button type="button" class="icon-btn h-9 w-9" aria-label="Fermer" @click="close">
						<AppIcon name="close" :size="17" />
					</button>
				</header>

				<div class="flex items-center gap-1.5 px-4 pt-3 text-[11px] font-semibold">
					<button
						type="button"
						class="rounded-full px-3 py-1 transition-normal"
						:class="step === 'choose' ? 'bg-brand/20 text-brandSoft' : 'bg-white/5 text-white/45'"
						@click="step = 'choose'"
					>
						{{ effectiveSeries ? "1 · Qualité & saison" : "1 · Qualité" }}
					</button>
					<button
						v-if="effectiveSeries"
						type="button"
						class="rounded-full px-3 py-1 transition-normal"
						:class="step === 'episodes' ? 'bg-brand/20 text-brandSoft' : 'bg-white/5 text-white/45'"
						@click="goToEpisodes"
					>
						2 · Épisodes
					</button>
				</div>

				<div class="min-h-0 flex-1 overflow-y-auto px-4 py-4">
					<div v-if="isLoadingQuality" class="space-y-2">
						<div v-for="index in 3" :key="`q-${index}`" class="skeleton h-12 w-full"></div>
					</div>

					<!-- Étape 1 : qualité (+ saison pour une série) -->
					<template v-else-if="step === 'choose'">
						<p class="mb-3 text-xs text-white/45">
							{{
								effectiveSeries
									? "Choisis la qualité et la saison, puis touche « Avancer »."
									: "Choisis la qualité de la vidéo à enregistrer."
							}}
						</p>

						<p class="mb-2 text-[11px] font-semibold uppercase tracking-wider text-white/35">Qualité</p>
						<div class="space-y-2">
							<button
								v-for="quality in qualityOptions"
								:key="quality"
								type="button"
								class="flex w-full items-center justify-between gap-3 rounded-xl border px-3.5 py-3 text-left transition-normal"
								:class="
									selectedQuality === quality
										? 'border-brand/50 bg-brand/10'
										: 'border-inkLine bg-ink hover:border-white/25'
								"
								@click="selectQuality(quality)"
							>
								<span class="flex items-center gap-3">
									<span
										class="grid h-8 w-8 place-items-center rounded-lg text-xs font-bold"
										:class="selectedQuality === quality ? 'bg-brand text-white' : 'bg-white/5 text-white/60'"
									>
										{{ quality.slice(0, 3) }}
									</span>
									<span class="text-sm font-semibold text-white">{{ quality }}</span>
								</span>
								<span class="flex items-center gap-2 text-[11px] text-white/40">
									<span v-if="!effectiveSeries && movieStream?.size">{{ formatSize(movieStream.size) }}</span>
									<AppIcon v-if="selectedQuality === quality" name="check" :size="16" class="text-brandSoft" />
								</span>
							</button>
						</div>

						<template v-if="effectiveSeries">
							<p class="mb-2 mt-4 text-[11px] font-semibold uppercase tracking-wider text-white/35">Saison</p>
							<div class="rail -mx-1 px-1">
								<button
									v-for="season in seasons"
									:key="season"
									type="button"
									class="chip"
									:class="selectedSeason === season ? 'chip-active' : ''"
									@click="((selectedSeason = season), (selectedKeys = []))"
								>
									Saison {{ season }}
								</button>
							</div>
						</template>
					</template>

					<!-- Étape 2 : épisodes -->
					<template v-else>
						<div class="mb-3 flex items-center justify-between gap-3">
							<p class="text-xs text-white/45">
								Saison {{ selectedSeason }} · {{ selectedQuality }} · {{ selectedCount }} sélectionné(s)
							</p>
							<button type="button" class="chip !py-1 text-xs" @click="toggleAll">
								{{ allSelected ? "Tout désélectionner" : "Tout sélectionner" }}
							</button>
						</div>

						<div v-if="!seasonEpisodes.length" class="rounded-xl border border-inkLine bg-ink p-4 text-center text-xs text-white/45">
							Aucun épisode pour cette saison.
						</div>

						<ul v-else class="space-y-1.5">
							<li v-for="entry in seasonEpisodes" :key="keyOf(entry.episode)">
								<button
									type="button"
									class="flex w-full items-center gap-3 rounded-xl border px-3 py-2.5 text-left transition-normal disabled:opacity-40"
									:class="
										selectedKeys.includes(keyOf(entry.episode))
											? 'border-brand/50 bg-brand/10'
											: 'border-inkLine bg-ink hover:border-white/25'
									"
									:disabled="!entry.available"
									@click="toggleEpisode(entry)"
								>
									<span
										class="grid h-6 w-6 shrink-0 place-items-center rounded-md border text-white"
										:class="
											selectedKeys.includes(keyOf(entry.episode))
												? 'border-brand bg-brand'
												: 'border-white/25 bg-transparent'
										"
									>
										<AppIcon
											v-if="selectedKeys.includes(keyOf(entry.episode))"
											name="check"
											:size="13"
										/>
									</span>
									<span class="min-w-0 flex-1">
										<span class="block text-xs font-bold text-brandSoft">{{ entry.episode.label }}</span>
										<span class="line-clamp-1 block text-sm text-white/85">
											{{ entry.episode.name || entry.episode.title || `Épisode ${entry.episode.number}` }}
										</span>
									</span>
									<span
										v-if="!entry.available"
										class="shrink-0 text-[10px] uppercase tracking-wide text-amber-300/80"
									>
										Indispo.
									</span>
									<span v-else-if="entry.stream?.size" class="shrink-0 text-[10px] text-white/35">
										{{ formatSize(entry.stream.size) }}
									</span>
								</button>
							</li>
						</ul>
					</template>
				</div>

				<footer class="flex items-center gap-2 border-t border-white/5 px-4 py-3.5">
					<button
						v-if="step !== 'choose'"
						type="button"
						class="btn btn-inverse px-4 py-2.5"
						@click="goBack"
					>
						<AppIcon name="chevronLeft" :size="16" />
						<span>Retour</span>
					</button>

					<button
						type="button"
						class="btn btn-solid ml-auto px-5 py-2.5"
						:disabled="isWorking || (step === 'episodes' && !selectedCount)"
						@click="onPrimary"
					>
						<AppIcon
							:name="effectiveSeries && step !== 'episodes' ? 'chevronRight' : 'download'"
							:size="16"
						/>
						<span>{{ isWorking ? "Démarrage…" : primaryLabel }}</span>
					</button>
				</footer>
			</div>
		</div>
	</Teleport>
</template>
