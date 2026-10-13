<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { downloadFileUrl } from "../api/client";
import { useDownloads } from "../stores/downloads";
import { toast } from "../stores/toast";
import { formatDate, formatEta, formatSize, formatSpeed } from "../util/format";
import AppIcon from "./AppIcon.vue";

/**
 * Ligne d'un telechargement : progression en direct, lecture locale et enregistrement.
 * La carte reste volontairement compacte (une action visible) : le reste des actions
 * est dans le menu « … » (detail, reprise, suppression).
 */
const props = defineProps({
	entry: { type: Object, required: true },
});

const { pause, retry, remove } = useDownloads();
const confirming = ref(false);
const isBusy = ref(false);
const menuOpen = ref(false);
const menuRef = ref(null);

const isActive = computed(() => ["queued", "downloading"].includes(props.entry.status));
const isDone = computed(() => props.entry.status === "completed");
const isFailed = computed(() => props.entry.status === "error");
const isPaused = computed(() => ["paused", "canceled"].includes(props.entry.status));

const heading = computed(() =>
	props.entry.kind === "episode" ? props.entry.episodeName || props.entry.title : props.entry.title,
);
const subheading = computed(() => (props.entry.kind === "episode" ? props.entry.seriesTitle : null));

const meta = computed(() =>
	[
		props.entry.kind === "episode" ? props.entry.label : null,
		props.entry.quality,
		formatSize(props.entry.bytesTotal || props.entry.bytesReceived),
	]
		.filter(Boolean)
		.join(" • "),
);

const liveLabel = computed(() => {
	if (props.entry.status === "queued") return "En attente dans la file";
	if (props.entry.status === "downloading") {
		return [
			formatSpeed(props.entry.speedBps),
			props.entry.etaSeconds ? `reste ${formatEta(props.entry.etaSeconds)}` : null,
		]
			.filter(Boolean)
			.join(" · ");
	}
	return null;
});

const statusLabel = computed(
	() =>
		({
			queued: "En attente",
			downloading: "Téléchargement",
			completed: "Téléchargé",
			error: "Interrompu",
			paused: "En pause",
			canceled: "Annulé",
		})[props.entry.status] || props.entry.status,
);

const badgeClass = computed(() => {
	if (isDone.value) return "badge badge-free";
	if (isFailed.value) return "badge bg-amber-500/20 text-amber-300 ring-1 ring-amber-500/30";
	if (isPaused.value) return "badge bg-white/10 text-white/60";
	return "badge badge-brand";
});

/** Detail complet, affiche par le menu « … ». */
const details = computed(() => {
	const entry = props.entry;
	const extension = String(entry.fileName || "").split(".").pop() || "—";
	return [
		{ label: "Fichier", value: entry.fileName || "—" },
		{ label: "Format", value: `${extension.toUpperCase()}${entry.format ? ` · ${entry.format}` : ""}` },
		{ label: "Qualité", value: entry.quality || "—" },
		{
			label: "Taille",
			value: `${formatSize(entry.bytesTotal || entry.bytesReceived)} (${Math.round(entry.progress)} %)`,
		},
		entry.kind === "episode"
			? { label: "Épisode", value: `${entry.seriesTitle || "—"} · ${entry.label || ""}` }
			: null,
		entry.completedAt ? { label: "Terminé le", value: formatDate(entry.completedAt) } : null,
		entry.error ? { label: "Erreur", value: entry.error } : null,
	].filter(Boolean);
});

const closeMenu = (event) => {
	if (menuRef.value && !menuRef.value.contains(event.target)) menuOpen.value = false;
};

onMounted(() => document.addEventListener("click", closeMenu));
onBeforeUnmount(() => document.removeEventListener("click", closeMenu));

const onPause = async () => {
	menuOpen.value = false;
	isBusy.value = true;
	try {
		await pause(props.entry.id);
		toast.info(`Téléchargement mis en pause — ${heading.value}`);
	} catch (error) {
		toast.error(error.message);
	} finally {
		isBusy.value = false;
	}
};

const onRetry = async () => {
	menuOpen.value = false;
	isBusy.value = true;
	try {
		await retry(props.entry.id);
		toast.success(`Reprise — ${heading.value}`);
	} catch (error) {
		toast.error(error.message);
	} finally {
		isBusy.value = false;
	}
};

const onRemove = async () => {
	// Premier clic : demande de confirmation ; second clic : suppression.
	if (!confirming.value) {
		confirming.value = true;
		window.setTimeout(() => {
			confirming.value = false;
		}, 4000);
		return;
	}

	menuOpen.value = false;
	confirming.value = false;
	isBusy.value = true;
	try {
		await remove(props.entry.id);
		toast.info(`« ${heading.value} » supprimé du stockage`);
	} catch (error) {
		toast.error(error.message);
	} finally {
		isBusy.value = false;
	}
};
</script>

<template>
	<li class="surface p-3 transition-normal hover:border-white/15">
		<div class="flex items-center gap-3">
			<RouterLink
				:to="isDone && props.entry.playable ? { name: 'watch-local', params: { id: props.entry.id } } : '/downloads'"
				class="relative h-14 w-24 shrink-0 overflow-hidden rounded-lg bg-inkCard ring-1 ring-white/5"
			>
				<img
					v-if="props.entry.poster"
					:src="props.entry.poster"
					:alt="heading"
					loading="lazy"
					class="h-full w-full object-cover"
				/>
				<span v-else class="grid h-full w-full place-items-center text-white/30">
					<AppIcon name="download" :size="18" />
				</span>
			</RouterLink>

			<div class="min-w-0 flex-1">
				<div class="flex items-center gap-2">
					<span :class="badgeClass">{{ statusLabel }}</span>
					<span v-if="isDone" class="text-[11px] text-white/40">100 %</span>
					<span v-else-if="isActive" class="text-xs font-semibold text-brandSoft">
						{{ props.entry.progress }} %
					</span>
				</div>

				<h3 class="line-clamp-1 text-sm font-semibold text-white/90">{{ heading }}</h3>
				<p v-if="subheading" class="line-clamp-1 text-[11px] text-white/45">{{ subheading }}</p>
				<p class="line-clamp-1 text-[11px] text-white/40">{{ meta }}</p>
				<p v-if="liveLabel" class="line-clamp-1 text-[11px] text-white/55">{{ liveLabel }}</p>
				<p v-if="isFailed && props.entry.error" class="line-clamp-1 text-[11px] text-amber-300/90">
					{{ props.entry.error }}
				</p>
			</div>

			<!-- Une seule action visible : lire quand le fichier est pret. -->
			<RouterLink
				v-if="isDone && props.entry.playable"
				:to="{ name: 'watch-local', params: { id: props.entry.id } }"
				class="btn btn-inverse shrink-0 px-3 py-2 text-xs"
			>
				<AppIcon name="play" :size="14" filled />
				<span class="hidden sm:inline">Regarder</span>
			</RouterLink>

			<!-- Menu « … » : detail, reprise, suppression. -->
			<div ref="menuRef" class="relative shrink-0">
				<button
					type="button"
					class="icon-btn h-9 w-9"
					:aria-label="`Actions pour ${heading}`"
					:aria-expanded="menuOpen"
					@click="menuOpen = !menuOpen"
				>
					<AppIcon name="more" :size="18" />
				</button>

				<div
					v-if="menuOpen"
					class="absolute right-0 top-11 z-20 w-64 space-y-2 rounded-2xl border border-inkLine bg-inkCard p-3 shadow-deck"
				>
					<p class="text-[11px] font-semibold uppercase tracking-wide text-white/35">Détail</p>
					<dl class="space-y-1">
						<div v-for="row in details" :key="row.label" class="flex gap-2 text-[11px]">
							<dt class="w-16 shrink-0 text-white/35">{{ row.label }}</dt>
							<dd class="min-w-0 flex-1 break-words text-white/75">{{ row.value }}</dd>
						</div>
					</dl>

					<div class="space-y-1.5 border-t border-white/5 pt-2">
						<a
							v-if="isDone"
							:href="downloadFileUrl(props.entry.id)"
							:download="props.entry.fileName"
							class="btn btn-inverse w-full px-3 py-2 text-xs"
							@click="menuOpen = false"
						>
							<AppIcon name="hardDrive" :size="14" />
							<span>Enregistrer sur l'appareil</span>
						</a>

						<RouterLink
							v-if="isDone && props.entry.playable"
							:to="{ name: 'watch-local', params: { id: props.entry.id } }"
							class="btn btn-inverse w-full px-3 py-2 text-xs sm:hidden"
							@click="menuOpen = false"
						>
							<AppIcon name="play" :size="14" filled />
							<span>Regarder</span>
						</RouterLink>

						<button
							v-if="isActive"
							type="button"
							class="btn btn-inverse w-full px-3 py-2 text-xs"
							:disabled="isBusy"
							@click="onPause"
						>
							<AppIcon name="pause" :size="14" />
							<span>Mettre en pause</span>
						</button>

						<button
							v-else
							type="button"
							class="btn btn-inverse w-full px-3 py-2 text-xs"
							:disabled="isBusy"
							@click="onRetry"
						>
							<AppIcon name="download" :size="14" />
							<span>Reprendre le téléchargement</span>
						</button>

						<button
							type="button"
							class="btn w-full px-3 py-2 text-xs"
							:class="confirming ? 'btn-solid' : 'btn-ghost text-rose'"
							:disabled="isBusy"
							@click="onRemove"
						>
							<AppIcon name="trash" :size="14" />
							<span>{{ confirming ? "Confirmer la suppression" : "Supprimer" }}</span>
						</button>
					</div>
				</div>
			</div>
		</div>

		<!-- Barre de progression sous la carte quand le transfert est actif. -->
		<div v-if="isActive || isPaused" class="mt-2 h-1 overflow-hidden rounded-full bg-white/10">
			<div class="h-full bg-brand-gradient transition-normal" :style="{ width: `${props.entry.progress}%` }"></div>
		</div>
	</li>
</template>
