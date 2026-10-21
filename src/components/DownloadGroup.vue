<script setup>
import { computed, ref } from "vue";
import { downloadFileUrl } from "../api/client";
import { useDownloads } from "../stores/downloads";
import { toast } from "../stores/toast";
import { formatSize } from "../util/format";
import ActionMenu from "./ActionMenu.vue";
import AppIcon from "./AppIcon.vue";

/**
 * Dossier d'une série : regroupe tous ses épisodes téléchargés sous une seule
 * carte, avec l'essentiel visible (nombre d'épisodes, progression, taille) et un
 * bouton « Lire le suivant ». Le clic sur la vignette déplie la liste ordonnée
 * des épisodes, chacun disposant de son propre menu « … ».
 */
const props = defineProps({
	group: { type: Object, required: true },
});

const emit = defineEmits(["remove-many"]);

const { pause, retry, remove } = useDownloads();

const expanded = ref(false);
const confirmingMany = ref(false);
const confirmingId = ref(null);
const busyId = ref(null);

const total = computed(() => props.group.entries.length);
const done = computed(() => props.group.entries.filter((entry) => entry.status === "completed"));
const active = computed(() =>
	props.group.entries.filter((entry) => ["queued", "downloading"].includes(entry.status)),
);
const failed = computed(() =>
	props.group.entries.filter((entry) => ["error", "paused", "canceled"].includes(entry.status)),
);
const seasonLabel = computed(() => {
	const numbers = props.group.entries
		.map((entry) => entry.seasonNumber)
		.filter((value) => Number.isFinite(value));
	if (!numbers.length) return null;
	const min = Math.min(...numbers);
	const max = Math.max(...numbers);
	return min === max ? `Saison ${min}` : `Saisons ${min} – ${max}`;
});

const totalBytes = computed(() =>
	props.group.entries.reduce((sum, entry) => sum + (entry.bytesTotal || entry.bytesReceived || 0), 0),
);
// Progression en octets : une barre qui n'avance qu'au nombre d'épisodes terminés
// restait immobile pendant un téléchargement multiple (tous les épisodes en cours).
const progress = computed(() => {
	const entries = props.group.entries;
	let totalBytes = 0;
	let receivedBytes = 0;
	for (const entry of entries) {
		const size = entry.bytesTotal || entry.bytesReceived || 0;
		totalBytes += size;
		receivedBytes +=
			entry.status === "completed" ? size : Math.min(entry.bytesReceived || 0, size);
	}
	if (totalBytes > 0) return Math.min(100, Math.round((receivedBytes / totalBytes) * 100));
	return total.value ? Math.round((done.value.length / total.value) * 100) : 0;
});
const statusLabel = computed(() => {
	if (active.value.length) return `${done.value.length}/${total.value} · ${active.value.length} en cours`;
	if (failed.value.length) return `${done.value.length}/${total.value} · ${failed.value.length} en échec`;
	return `${done.value.length}/${total.value} épisode${total.value > 1 ? "s" : ""} téléchargé${done.value.length > 1 ? "s" : ""}`;
});

const poster = computed(() => {
	const withPoster = props.group.entries.find((entry) => entry.poster);
	return withPoster?.poster || null;
});

/**
 * Fiche de la série d'origine, reconstruite depuis `sourceId` :
 * `internal:<id>:<saison>:<épisode>` ou `<archiveId>` pour une archive libre.
 */
const seriesTarget = computed(() => {
	const entry = props.group.entries.find((item) => item.sourceId);
	const raw = String(entry?.sourceId || "");
	const internal = raw.match(/^internal:([^:]+)/);
	if (internal) return { mediaType: "internal", id: internal[1] };

	const base = raw.replace(/:\d+:\d+$/, "");
	return base ? { mediaType: "archive", id: base } : null;
});

/** Saison la plus récente du dossier : celle que l'utilisateur complète. */
const latestSeason = computed(() => {
	const numbers = props.group.entries
		.map((entry) => entry.seasonNumber)
		.filter((value) => Number.isFinite(value));
	return numbers.length ? Math.max(...numbers) : null;
});

/** Ouvre l'assistant de téléchargement sur les épisodes non possédés. */
const missingRoute = computed(() =>
	seriesTarget.value
		? {
				name: "detail",
				params: seriesTarget.value,
				query: {
					download: "missing",
					...(latestSeason.value ? { season: String(latestSeason.value) } : {}),
				},
			}
		: null,
);

/**
 * Épisode à lire ensuite : le premier dans l'ordre (saison, numéro) venant après
 * le plus récemment terminé — puis boucle au début (relecture d'une saison).
 */
const nextEntry = computed(() => {
	const list = props.group.entries;
	const finished = done.value;
	if (!finished.length) return null;
	if (finished.length === 1) return finished[0];

	const latest = finished.reduce((a, b) =>
		(a.completedAt || 0) >= (b.completedAt || 0) ? a : b,
	);
	const index = list.indexOf(latest);
	for (let i = index + 1; i < list.length; i += 1) {
		if (list[i].status === "completed") return list[i];
	}
	return finished[0];
});

const watchedLabel = computed(() => {
	const entry = nextEntry.value;
	if (!entry) return "Aucun épisode prêt";
	const current = done.value.reduce((a, b) =>
		(a.completedAt || 0) >= (b.completedAt || 0) ? a : b,
	);
	return entry === current ? "Lire le premier épisode" : "Lire l'épisode suivant";
});

const episodeLabel = (entry) =>
	entry.label ||
	(entry.seasonNumber && entry.episodeNumber
		? `S${String(entry.seasonNumber).padStart(2, "0")}E${String(entry.episodeNumber).padStart(2, "0")}`
		: entry.title);

const isActive = (entry) => ["queued", "downloading"].includes(entry.status);
const isDone = (entry) => entry.status === "completed";

const onPause = async (entry) => {
	busyId.value = entry.id;
	try {
		await pause(entry.id);
		toast.info(`Téléchargement mis en pause — ${entry.episodeName || entry.title}`);
	} catch (error) {
		toast.error(error.message);
	} finally {
		busyId.value = null;
	}
};

const onRetry = async (entry) => {
	busyId.value = entry.id;
	try {
		await retry(entry.id);
		toast.success(`Reprise — ${entry.episodeName || entry.title}`);
	} catch (error) {
		toast.error(error.message);
	} finally {
		busyId.value = null;
	}
};

const onRemoveOne = async (entry, close) => {
	if (confirmingId.value !== entry.id) {
		confirmingId.value = entry.id;
		window.setTimeout(() => {
			if (confirmingId.value === entry.id) confirmingId.value = null;
		}, 4000);
		return;
	}

	confirmingId.value = null;
	busyId.value = entry.id;
	close?.();
	try {
		await remove(entry.id);
		toast.info(`« ${entry.episodeName || entry.title} » supprimé`);
	} catch (error) {
		toast.error(error.message);
	} finally {
		busyId.value = null;
	}
};

const onRemoveMany = (close) => {
	// Premier clic : demande de confirmation ; second clic : le parent supprime.
	if (!confirmingMany.value) {
		confirmingMany.value = true;
		window.setTimeout(() => {
			confirmingMany.value = false;
		}, 4000);
		return;
	}

	confirmingMany.value = false;
	close?.();
	emit("remove-many", props.group.entries.map((entry) => entry.id));
};
</script>

<template>
	<li class="surface transition-normal hover:border-white/15">
		<!-- Carte du dossier -->
		<div class="flex items-center gap-3 p-3">
			<button
				type="button"
				class="relative h-14 w-24 shrink-0 overflow-hidden rounded-lg bg-inkCard ring-1 ring-white/5"
				:aria-expanded="expanded"
				@click="expanded = !expanded"
			>
				<img
					v-if="poster"
					:src="poster"
					:alt="group.title"
					loading="lazy"
					class="h-full w-full object-cover"
				/>
				<span v-else class="grid h-full w-full place-items-center text-white/30">
					<AppIcon name="tv" :size="20" />
				</span>
				<span
					class="absolute inset-0 grid place-items-center bg-black/45 transition-normal"
					:class="expanded ? 'opacity-0' : 'opacity-100'"
				>
					<AppIcon name="list" :size="18" class="text-white/70" />
				</span>
			</button>

			<div class="min-w-0 flex-1">
				<div class="flex items-center gap-2">
					<span class="badge badge-brand">Série</span>
					<span v-if="seasonLabel" class="text-[11px] text-white/40">{{ seasonLabel }}</span>
					<span class="text-[11px] text-white/40">{{ formatSize(totalBytes) }}</span>
				</div>

				<h3 class="line-clamp-1 text-sm font-semibold text-white/90">{{ group.title }}</h3>
				<p class="line-clamp-1 text-[11px] text-white/45">
					{{ statusLabel }}
					<span v-if="active.length"> · en cours…</span>
				</p>

				<!-- Barre de progression globale du dossier -->
				<div class="mt-1.5 h-1 overflow-hidden rounded-full bg-white/10">
					<div class="h-full bg-brand-gradient transition-normal" :style="{ width: `${progress}%` }"></div>
				</div>
			</div>

			<!-- L'épisode suivant en un clic -->
			<RouterLink
				v-if="nextEntry"
				:to="{ name: 'watch-local', params: { id: nextEntry.id } }"
				class="btn btn-solid shrink-0 px-3 py-2 text-xs"
			>
				<AppIcon name="play" :size="14" filled />
				<span class="hidden sm:inline">{{ watchedLabel }}</span>
				<span class="sm:hidden">Suivant</span>
			</RouterLink>

			<RouterLink
				v-if="missingRoute"
				:to="missingRoute"
				class="icon-btn h-9 w-9 shrink-0 text-brandSoft"
				title="Télécharger les épisodes manquants"
				aria-label="Télécharger les épisodes manquants"
			>
				<AppIcon name="download" :size="17" />
			</RouterLink>

			<ActionMenu :label="`Actions pour ${group.title}`" :width="240">
				<template #default="{ close }">
					<RouterLink
						v-if="nextEntry"
						:to="{ name: 'watch-local', params: { id: nextEntry.id } }"
						class="menu-item"
					>
						<AppIcon name="play" :size="15" filled />
						<span>{{ watchedLabel }}</span>
					</RouterLink>

					<RouterLink v-if="missingRoute" :to="missingRoute" class="menu-item">
						<AppIcon name="download" :size="15" />
						<span>Télécharger les épisodes manquants</span>
					</RouterLink>

					<button type="button" class="menu-item" @click="expanded = !expanded">
						<AppIcon name="list" :size="15" />
						<span>{{ expanded ? "Masquer les épisodes" : "Afficher les épisodes" }}</span>
					</button>

					<div class="menu-separator"></div>

					<button
						type="button"
						data-keep-open
						class="menu-item menu-item-danger"
						@click="onRemoveMany(close)"
					>
						<AppIcon name="trash" :size="15" />
						<span>{{ confirmingMany ? "Confirmer : tout supprimer" : "Supprimer toute la série" }}</span>
					</button>
				</template>
			</ActionMenu>
		</div>

		<!-- Liste ordonnée des épisodes -->
		<Transition
			enter-active-class="transition-all duration-300 ease-out"
			leave-active-class="transition-all duration-200 ease-in"
			enter-from-class="opacity-0 max-h-0"
			enter-to-class="opacity-100 max-h-[600px]"
			leave-from-class="opacity-100 max-h-[600px]"
			leave-to-class="opacity-0 max-h-0"
		>
			<div v-if="expanded" class="border-t border-white/5">
				<ul class="max-h-[60vh] divide-y divide-white/5 overflow-y-auto px-3">
					<li v-for="entry in group.entries" :key="entry.id">
						<div class="flex items-center gap-2 py-2.5">
							<span
								class="grid h-9 w-14 shrink-0 place-items-center rounded-lg text-[11px] font-bold"
								:class="
									isDone(entry)
										? 'bg-brand/15 text-brandSoft'
										: isActive(entry)
											? 'bg-white/10 text-white/80'
											: 'bg-white/5 text-white/40'
								"
							>
								{{ episodeLabel(entry) }}
							</span>

							<div class="min-w-0 flex-1">
								<p class="line-clamp-1 text-xs font-semibold text-white/85">
									{{ entry.episodeName || entry.title }}
								</p>
								<p class="text-[11px] text-white/40">
									{{ entry.quality }} · {{ formatSize(entry.bytesTotal || entry.bytesReceived) }}
									<span v-if="entry.status === 'downloading'"> · {{ entry.progress }} %</span>
									<span v-if="entry.status === 'error'" class="text-amber-300/90"> · interrompu</span>
								</p>
								<div
									v-if="isActive(entry)"
									class="mt-1 h-1 overflow-hidden rounded-full bg-white/10"
								>
									<div
										class="h-full bg-brand-gradient transition-normal"
										:style="{ width: `${entry.progress}%` }"
									></div>
								</div>
							</div>

							<!-- Chaque fichier a son propre menu « … ». -->
							<ActionMenu :label="`Actions pour ${entry.episodeName || entry.title}`" :width="224">
								<template #default="{ close }">
									<RouterLink
										v-if="entry.playable"
										:to="{ name: 'watch-local', params: { id: entry.id } }"
										class="menu-item"
									>
										<AppIcon name="play" :size="15" filled />
										<span>{{ isDone(entry) ? "Lire" : "Lire en cours" }}</span>
									</RouterLink>

									<a
										v-if="isDone(entry)"
										:href="downloadFileUrl(entry.id)"
										:download="entry.fileName"
										class="menu-item"
									>
										<AppIcon name="hardDrive" :size="15" />
										<span>Enregistrer sur l'appareil</span>
									</a>

									<button
										v-if="isActive(entry)"
										type="button"
										class="menu-item"
										:disabled="busyId === entry.id"
										@click="onPause(entry)"
									>
										<AppIcon name="pause" :size="15" />
										<span>Mettre en pause</span>
									</button>

									<button
										v-else-if="!isDone(entry)"
										type="button"
										class="menu-item"
										:disabled="busyId === entry.id"
										@click="onRetry(entry)"
									>
										<AppIcon name="download" :size="15" />
										<span>Reprendre le téléchargement</span>
									</button>

									<div class="menu-separator"></div>

									<button
										type="button"
										data-keep-open
										class="menu-item menu-item-danger"
										:disabled="busyId === entry.id"
										@click="onRemoveOne(entry, close)"
									>
										<AppIcon name="trash" :size="15" />
										<span>{{ confirmingId === entry.id ? "Confirmer la suppression" : "Supprimer" }}</span>
									</button>
								</template>
							</ActionMenu>
						</div>
					</li>
				</ul>
			</div>
		</Transition>
	</li>
</template>
