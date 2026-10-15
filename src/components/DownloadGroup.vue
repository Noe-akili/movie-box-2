<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { downloadFileUrl } from "../api/client";
import { toast } from "../stores/toast";
import { formatSize } from "../util/format";
import AppIcon from "./AppIcon.vue";

/**
 * Dossier d'une série : regroupe tous ses épisodes téléchargés sous une seule
 * carte (comme un dossier de fichiers), avec l'essentiel visibles : nombre
 * d'épisodes, progression, taille — et un bouton « Lire le suivant ».
 * Le clic déplie la liste ordonnée des épisodes (S1 E1, S1 E2 …).
 */
const props = defineProps({
	group: { type: Object, required: true },
});

const emit = defineEmits(["remove-many"]);

const expanded = ref(false);
const menuOpen = ref(false);
const menuRef = ref(null);
const confirming = ref(false);

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
const progress = computed(() =>
	total.value ? Math.round((done.value.length / total.value) * 100) : 0,
);
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
	// L'épisode suivant dans l'ordre naturel, sinon retour au premier téléchargé.
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

const closeMenu = (event) => {
	if (menuRef.value && !menuRef.value.contains(event.target)) menuOpen.value = false;
};

onMounted(() => document.addEventListener("click", closeMenu));
onBeforeUnmount(() => document.removeEventListener("click", closeMenu));

const toggleMenu = () => {
	menuOpen.value = !menuOpen.value;
};

const onRemoveMany = () => {
	// Premier clic : demande de confirmation ; second clic : le parent supprime.
	if (!confirming.value) {
		confirming.value = true;
		window.setTimeout(() => {
			confirming.value = false;
		}, 4000);
		return;
	}

	menuOpen.value = false;
	confirming.value = false;
	emit("remove-many", props.group.entries.map((entry) => entry.id));
};
</script>

<template>
	<li class="surface overflow-hidden transition-normal hover:border-white/15">
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
					class="absolute inset-0 grid place-items-center bg-black/45 opacity-0 transition-normal"
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

			<div ref="menuRef" class="relative shrink-0">
				<button
					type="button"
					class="icon-btn h-9 w-9"
					:aria-label="`Actions pour ${group.title}`"
					:aria-expanded="menuOpen"
					@click.stop="toggleMenu"
				>
					<AppIcon name="more" :size="18" />
				</button>

				<div
					v-if="menuOpen"
					class="absolute right-0 top-11 z-20 w-60 space-y-2 rounded-2xl border border-inkLine bg-inkCard p-3 shadow-deck"
				>
					<p class="text-[11px] font-semibold uppercase tracking-wide text-white/35">Détail</p>
					<dl class="space-y-1">
						<div class="flex gap-2 text-[11px]">
							<dt class="w-16 shrink-0 text-white/35">Titre</dt>
							<dd class="min-w-0 flex-1 break-words text-white/75">{{ group.title }}</dd>
						</div>
						<div class="flex gap-2 text-[11px]">
							<dt class="w-16 shrink-0 text-white/35">Épisodes</dt>
							<dd class="text-white/75">{{ done.length }}/{{ total }} terminés</dd>
						</div>
						<div v-if="seasonLabel" class="flex gap-2 text-[11px]">
							<dt class="w-16 shrink-0 text-white/35">Saisons</dt>
							<dd class="text-white/75">{{ seasonLabel }}</dd>
						</div>
						<div class="flex gap-2 text-[11px]">
							<dt class="w-16 shrink-0 text-white/35">Taille</dt>
							<dd class="text-white/75">{{ formatSize(totalBytes) }}</dd>
						</div>
					</dl>

					<button
						type="button"
						class="btn w-full px-3 py-2 text-xs"
						:class="confirming ? 'btn-solid' : 'btn-ghost text-rose'"
						@click="onRemoveMany"
					>
						<AppIcon name="trash" :size="14" />
						<span>{{ confirming ? "Confirmer : tout supprimer" : "Supprimer toute la série" }}</span>
					</button>
				</div>
			</div>
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
						<div class="flex items-center gap-3 py-2.5">
							<span
								class="grid h-9 w-14 shrink-0 place-items-center rounded-lg text-[11px] font-bold"
								:class="
									entry.status === 'completed'
										? 'bg-brand/15 text-brandSoft'
										: ['queued', 'downloading'].includes(entry.status)
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
							</div>

							<RouterLink
								v-if="entry.status === 'completed' && entry.playable"
								:to="{ name: 'watch-local', params: { id: entry.id } }"
								class="icon-btn h-8 w-8 shrink-0"
								:aria-label="`Lire ${entry.episodeName || entry.title}`"
							>
								<AppIcon name="play" :size="15" filled />
							</RouterLink>

							<a
								v-else-if="entry.status === 'completed'"
								:href="downloadFileUrl(entry.id)"
								:download="entry.fileName"
								class="icon-btn h-8 w-8 shrink-0"
								:aria-label="`Enregistrer ${entry.fileName}`"
							>
								<AppIcon name="hardDrive" :size="15" />
							</a>

							<span v-else class="text-[11px] text-white/35">
								{{ entry.status === 'downloading' ? `${entry.progress} %` : '—' }}
							</span>
						</div>
					</li>
				</ul>
			</div>
		</Transition>
	</li>
</template>
