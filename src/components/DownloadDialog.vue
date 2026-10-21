<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
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
import { isSeriesTitle } from "../util/media";
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
	/** Raccourci « épisodes manquants » : ouvre directement la sélection. */
	autoMissing: { type: Boolean, default: false },
	initialSeason: { type: Number, default: null },
});

const emit = defineEmits(["close"]);

const { start, items } = useDownloads();

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

// Type réel du titre : on s'appuie sur l'unique source de vérité `isSeriesTitle`
// (kind, mediaType, subjectType, saisons/épisodes). Un épisode explicite force la
// branche série. Une série gratuite ne doit jamais être prise pour un film.
const isSeries = computed(() => isSeriesTitle(props.item) || Boolean(props.episode));

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

/** Nombre total d'épisodes listés pour une saison (toutes qualités confondues). */
const seasonCount = (season) =>
	episodes.value.filter((episode) => (episode.season ?? 1) === season).length;

const step = ref("choose");
const selectedQuality = ref(null);
const selectedSeason = ref(null);
const selectedKeys = ref([]);
const qualityOptions = ref([]);
const isLoadingQuality = ref(false);
const isWorking = ref(false);

const keyOf = (episode) => `${episode.season ?? 1}:${episode.number ?? 1}`;

const qualityStreams = computed(() => {
	const list = Array.isArray(props.item?.streams) ? props.item.streams : [];
	if (list.length) return list;
	return props.stream ? [props.stream] : [];
});

const movieStream = computed(
	() => pickQualityStream(qualityStreams.value, selectedQuality.value) || props.stream || null,
);

// Chaque qualité a son propre poids : sans cette recherche par qualité, toutes
// les lignes affichaient la taille de la qualité actuellement sélectionnée.
const sizeForQuality = (quality) => {
	const stream = pickQualityStream(qualityStreams.value, quality);
	return typeof stream?.size === "number" && stream.size > 0 ? stream.size : null;
};

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
const selectedEpisodes = computed(() =>
	seasonEpisodes.value.filter(
		(entry) => entry.available && selectedKeys.value.includes(keyOf(entry.episode)),
	),
);

// Tailles : par épisode puis cumul de la sélection. L'API interne ne renvoie la
// taille qu'épisode par épisode (une saison récente n'a parfois qu'une qualité) :
// on interroge les flux au changement de saison pour afficher un total juste.
const episodeSizes = ref({});
const isLoadingSizes = ref(false);

const sizeOf = (episode) => {
	const key = keyOf(episode);
	const known = episodeSizes.value[key];
	return typeof known === "number" ? known : null;
};

const totalSize = computed(() => {
	let total = 0;
	let known = 0;
	for (const entry of selectedEpisodes.value) {
		const size = sizeOf(entry.episode);
		if (typeof size === "number") {
			total += size;
			known += 1;
		}
	}
	// Total partiel : on ne l'affiche que si au moins une taille est connue.
	return { total, known, complete: known > 0 && known === selectedEpisodes.value.length };
});

const totalSizeLabel = computed(() => {
	if (!totalSize.value.known) return null;
	const label = formatSize(totalSize.value.total);
	return totalSize.value.complete ? label : `environ ${label}`;
});

/** Lit les tailles des épisodes d'une saison (source interne uniquement). */
const loadEpisodeSizes = async (season) => {
	if (!isInternal.value || !internalId.value || !season) return;

	const pool = episodes.value.filter((episode) => (episode.season ?? 1) === season);
	const missing = pool.filter((episode) => sizeOf(episode) === null);
	if (!missing.length) return;

	isLoadingSizes.value = true;
	try {
		await Promise.all(
			missing.map(async (episode) => {
				try {
					const data = await fetchInternalStreams(internalId.value, {
						season: episode.season ?? 1,
						episode: episode.number ?? 1,
					});
					const stream = pickQualityStream(data?.streams || [], selectedQuality.value);
					if (stream?.size) {
						episodeSizes.value = {
							...episodeSizes.value,
							[keyOf(episode)]: stream.size,
						};
					}
				} catch {
					// Taille indisponible pour cet épisode : on l'ignore.
				}
			}),
		);
	} finally {
		isLoadingSizes.value = false;
	}
};
/**
 * Téléchargement déjà présent pour un épisode (même série, saison et numéro).
 * On s'appuie sur les métadonnées enregistrées plutôt que sur le nom de fichier :
 * les flux internes ne sont résolus qu'au moment du téléchargement.
 */
const downloadForEpisode = (episode) => {
	const season = episode.season ?? 1;
	const number = episode.number ?? 1;
	const series = itemTitle.value;
	return (
		items.value.find(
			(entry) =>
				entry.kind === "episode" &&
				entry.seasonNumber === season &&
				entry.episodeNumber === number &&
				(entry.seriesTitle || "") === series,
		) || null
	);
};

/** `downloaded` (possédé), `active` (en cours) ou `paused` (erreur/annulé). */
const episodeState = (episode) => {
	const entry = downloadForEpisode(episode);
	if (!entry) return null;
	if (entry.status === "completed") return "downloaded";
	if (entry.status === "queued" || entry.status === "downloading") return "active";
	return "paused";
};

const downloadedCount = computed(
	() => seasonEpisodes.value.filter((entry) => episodeState(entry.episode) === "downloaded").length,
);

/**
 * Numéro du dernier épisode déjà téléchargé dans une saison : le raccourci
 * « épisodes manquants » ne propose que les épisodes suivants, jamais ceux
 * situés avant un épisode déjà possédé.
 */
const lastDownloadedNumber = (season) =>
	episodes.value.reduce((highest, episode) => {
		if ((episode.season ?? 1) !== season) return highest;
		if (episodeState(episode) !== "downloaded") return highest;
		return Math.max(highest, episode.number ?? 0);
	}, 0);

const selectionFloor = computed(() => lastDownloadedNumber(selectedSeason.value));

/** Vrai si la saison a au moins un épisode non possédé après le dernier téléchargé. */
const hasMissingAfterFloor = (season) => {
	const floor = lastDownloadedNumber(season);
	return episodes.value.some(
		(episode) =>
			(episode.season ?? 1) === season &&
			(episode.number ?? 0) > floor &&
			episodeState(episode) !== "downloaded",
	);
};

const isInProgressSeason = (season) =>
	lastDownloadedNumber(season) > 0 && hasMissingAfterFloor(season);

/**
 * Saison à compléter en priorité : la dernière saison commencée (au moins un
 * épisode possédé) mais incomplète. On évite ainsi d'ouvrir une saison déjà
 * terminée et courte (ex. S6 = 10 épisodes) quand une autre saison est en cours.
 */
const firstMissingSeason = () => {
	const inProgress = [...seasons.value].reverse().find(isInProgressSeason);
	if (inProgress) return inProgress;

	return (
		seasons.value.find((season) =>
			episodes.value.some(
				(episode) =>
					(episode.season ?? 1) === season && episodeState(episode) !== "downloaded",
			),
		) ??
		seasons.value[0] ??
		1
	);
};

// Les épisodes déjà possédés ne sont pas proposés à la sélection (pas de doublon).
const selectableEpisodes = computed(() =>
	availableEpisodes.value.filter((entry) => episodeState(entry.episode) !== "downloaded"),
);

const allSelected = computed(
	() =>
		selectableEpisodes.value.length > 0 &&
		selectedCount.value === selectableEpisodes.value.length,
);

/** Film déjà téléchargé (titre + qualité), affiché sous le choix de qualité. */
const downloadedMovie = computed(() => {
	if (effectiveSeries.value) return null;
	return (
		items.value.find(
			(entry) =>
				entry.kind === "movie" &&
				entry.status === "completed" &&
				(entry.title || "") === itemTitle.value &&
				(!selectedQuality.value || entry.quality === selectedQuality.value),
		) || null
	);
});

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
	if (!entry.available || episodeState(entry.episode) === "downloaded") return;
	const key = keyOf(entry.episode);
	selectedKeys.value = selectedKeys.value.includes(key)
		? selectedKeys.value.filter((item) => item !== key)
		: [...selectedKeys.value, key];
};

const toggleAll = () => {
	selectedKeys.value = allSelected.value
		? []
		: selectableEpisodes.value.map((entry) => keyOf(entry.episode));
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

		// Certains épisodes n'existent pas dans la qualité choisie : on nomme et
		// on enregistre le fichier avec la qualité réellement résolue.
		const resolvedQuality = stream?.quality || quality;
		const fileName = downloadFileName(props.item, stream, {
			episode,
			quality: resolvedQuality,
		});

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
			quality: resolvedQuality,
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

const dialogRef = ref(null);
let previousFocus = null;

const FOCUSABLE =
	'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

/** Retient le focus clavier à l'intérieur de la modale (accessibilité). */
const trapFocus = (event) => {
	if (event.key !== "Tab" || !dialogRef.value) return;

	const nodes = Array.from(dialogRef.value.querySelectorAll(FOCUSABLE)).filter(
		(element) => element.offsetParent !== null || element === document.activeElement,
	);
	if (!nodes.length) return;

	const first = nodes[0];
	const last = nodes[nodes.length - 1];

	if (event.shiftKey && document.activeElement === first) {
		event.preventDefault();
		last.focus();
	} else if (!event.shiftKey && document.activeElement === last) {
		event.preventDefault();
		first.focus();
	}
};

const onKeydown = (event) => {
	if (event.key === "Escape") {
		close();
		return;
	}
	trapFocus(event);
};

// Changer de saison change les qualites disponibles : on les relit pour la
// saison affichée (une saison récente peut n'exister qu'en 480p), puis on lit
// la taille de chaque épisode pour alimenter le total.
watch(selectedSeason, (season) => {
	if (!effectiveSeries.value || !season) return;
	loadQualities(season);
	loadEpisodeSizes(season);
});

// La taille dépend aussi de la qualité choisie : on relit si elle change.
watch(selectedQuality, () => {
	if (!effectiveSeries.value || !selectedSeason.value) return;
	episodeSizes.value = {};
	loadEpisodeSizes(selectedSeason.value);
});

onMounted(async () => {
	previousFocus = document.activeElement;
	document.body.style.overflow = "hidden";
	window.addEventListener("keydown", onKeydown);
	await nextTick();
	dialogRef.value?.focus();

	if (effectiveSeries.value) {
		if (props.autoMissing) {
			// L'indice du dossier (dernière saison possédée) peut être une saison
			// déjà complète : on ne l'ouvre que s'il reste des épisodes après le
			// dernier téléchargé, sinon on choisit la saison en cours.
			const wanted = Number(props.initialSeason);
			selectedSeason.value =
				Number.isFinite(wanted) &&
				seasons.value.includes(wanted) &&
				hasMissingAfterFloor(wanted)
					? wanted
					: firstMissingSeason();
			step.value = "episodes";
		} else {
			selectedSeason.value = props.episode?.season ?? seasons.value[0] ?? 1;
			if (props.episode && episodeState(props.episode) !== "downloaded") {
				selectedKeys.value = [keyOf(props.episode)];
			}
		}
	}

	await loadQualities(selectedSeason.value);
	loadEpisodeSizes(selectedSeason.value);

	// Les épisodes manquants sont pré-sélectionnés une fois la saison chargée :
	// l'utilisateur n'a plus qu'à confirmer.
	if (props.autoMissing) {
		const floor = selectionFloor.value;
		selectedKeys.value = selectableEpisodes.value
			.filter((entry) => (entry.episode.number ?? 0) > floor)
			.map((entry) => keyOf(entry.episode));
	}
});

onBeforeUnmount(() => {
	document.body.style.overflow = "";
	window.removeEventListener("keydown", onKeydown);
	previousFocus?.focus?.();
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
				ref="dialogRef"
				tabindex="-1"
				class="relative flex max-h-[88vh] w-full flex-col overflow-hidden rounded-t-3xl border border-inkLine bg-inkCard shadow-deck outline-none sm:max-w-lg sm:rounded-3xl"
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
									<span v-if="!effectiveSeries && sizeForQuality(quality)">{{ formatSize(sizeForQuality(quality)) }}</span>
									<AppIcon v-if="selectedQuality === quality" name="check" :size="16" class="text-brandSoft" />
								</span>
							</button>
						</div>

						<p
							v-if="downloadedMovie"
							class="mt-3 flex items-center gap-1.5 text-xs font-semibold text-emerald-300/90"
						>
							<AppIcon name="check" :size="14" />
							<span>Déjà téléchargé{{ downloadedMovie.quality ? ` (${downloadedMovie.quality})` : "" }}</span>
						</p>

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
									<span class="text-white/40">({{ seasonCount(season) }})</span>
								</button>
							</div>
						</template>
					</template>

					<!-- Étape 2 : épisodes -->
					<template v-else>
						<div class="mb-3 flex flex-wrap items-center justify-between gap-3">
							<p class="text-xs text-white/45">
								Saison {{ selectedSeason }} · {{ selectedQuality }} · {{ selectedCount }} sélectionné(s)
								<span v-if="downloadedCount" class="text-emerald-300/80">
									· {{ downloadedCount }} déjà téléchargé{{ downloadedCount > 1 ? "s" : "" }}
								</span>
								<span v-if="autoMissing && selectionFloor" class="text-white/35">
									· à partir de l'épisode {{ selectionFloor + 1 }}
								</span>
								<span class="text-white/35">
									· {{ seasonEpisodes.length }} épisode{{ seasonEpisodes.length > 1 ? "s" : "" }}
								</span>
							</p>
							<p
								v-if="selectedCount"
								class="rounded-lg bg-brand/10 px-2.5 py-1 text-[11px] font-semibold text-brandSoft"
							>
								<span v-if="totalSizeLabel">Total : {{ totalSizeLabel }}</span>
								<span v-else-if="isLoadingSizes" class="text-white/40">Calcul de la taille…</span>
								<span v-else class="text-white/40">Taille inconnue</span>
							</p>
							<button
								type="button"
								class="chip !py-1 text-xs disabled:opacity-40"
								:disabled="!selectableEpisodes.length"
								@click="toggleAll"
							>
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
									:disabled="!entry.available || episodeState(entry.episode) === 'downloaded'"
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
									<span
										v-if="episodeState(entry.episode) === 'downloaded'"
										class="shrink-0 rounded-md bg-emerald-500/15 px-1.5 py-0.5 text-[10px] font-semibold text-emerald-300"
									>
										Possédé
									</span>
									<span
										v-else-if="episodeState(entry.episode) === 'active'"
										class="shrink-0 rounded-md bg-brand/15 px-1.5 py-0.5 text-[10px] font-semibold text-brandSoft"
									>
										En cours
									</span>
									<span
										v-if="entry.available && (entry.stream?.size || sizeOf(entry.episode) !== null)"
										class="shrink-0 text-[10px] text-white/35"
									>
										{{ formatSize(entry.stream?.size ?? sizeOf(entry.episode)) }}
									</span>
									<span
										v-else-if="entry.available && isLoadingSizes"
										class="skeleton h-3 w-12 shrink-0"
									></span>
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
