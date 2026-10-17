<script setup>
import { computed, ref } from "vue";
import { downloadFileUrl } from "../api/client";
import { useDownloads } from "../stores/downloads";
import { toast } from "../stores/toast";
import { formatDate, formatEta, formatSize, formatSpeed } from "../util/format";
import ActionMenu from "./ActionMenu.vue";
import AppIcon from "./AppIcon.vue";

/**
 * Ligne d'un telechargement : progression en direct, lecture locale et actions.
 * La carte reste compacte : une action visible (« Regarder ») et un menu « … »
 * pour le reste (enregistrer, pause/reprise, suppression).
 */
const props = defineProps({
	entry: { type: Object, required: true },
});

const { pause, retry, remove } = useDownloads();
const confirming = ref(false);
const isBusy = ref(false);

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

/** Petite ligne discrète : format et date de fin (quand connus). */
const subtitle = computed(() => {
	const extension = String(props.entry.fileName || "").split(".").pop() || null;
	return [
		extension ? extension.toUpperCase() : null,
		isDone.value && props.entry.completedAt ? formatDate(props.entry.completedAt) : null,
	]
		.filter(Boolean)
		.join(" • ");
});

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

const onPause = async () => {
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

/** Suppression en deux temps, déclenchée depuis le menu « … ». */
const onRemove = async (close) => {
	if (!confirming.value) {
		confirming.value = true;
		window.setTimeout(() => {
			confirming.value = false;
		}, 4000);
		return;
	}

	confirming.value = false;
	isBusy.value = true;
	close?.();
	try {
		await remove(props.entry.id);
		toast.info(`« ${heading.value} » supprimé`);
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
				<p v-if="subtitle" class="line-clamp-1 text-[11px] text-white/30">{{ subtitle }}</p>
				<p v-if="liveLabel" class="line-clamp-1 text-[11px] text-white/55">{{ liveLabel }}</p>
				<p v-if="isFailed && props.entry.error" class="line-clamp-1 text-[11px] text-amber-300/90">
					{{ props.entry.error }}
				</p>
			</div>

			<!-- Une seule action visible : lire quand le fichier est prêt. -->
			<RouterLink
				v-if="isDone && props.entry.playable"
				:to="{ name: 'watch-local', params: { id: props.entry.id } }"
				class="btn btn-inverse shrink-0 px-3 py-2 text-xs"
			>
				<AppIcon name="play" :size="14" filled />
				<span class="hidden sm:inline">Regarder</span>
			</RouterLink>

			<!-- Menu « … » : enregistrer, pause/reprise, suppression. -->
			<ActionMenu :label="`Actions pour ${heading}`" :width="224">
				<template #default="{ close }">
					<a
						v-if="isDone"
						:href="downloadFileUrl(props.entry.id)"
						:download="props.entry.fileName"
						class="menu-item"
					>
						<AppIcon name="hardDrive" :size="15" />
						<span>Enregistrer sur l'appareil</span>
					</a>

					<button
						v-if="isActive"
						type="button"
						class="menu-item"
						:disabled="isBusy"
						@click="onPause"
					>
						<AppIcon name="pause" :size="15" />
						<span>Mettre en pause</span>
					</button>

					<button
						v-else-if="!isDone"
						type="button"
						class="menu-item"
						:disabled="isBusy"
						@click="onRetry"
					>
						<AppIcon name="download" :size="15" />
						<span>Reprendre le téléchargement</span>
					</button>

					<div class="menu-separator"></div>

					<button
						type="button"
						data-keep-open
						class="menu-item menu-item-danger"
						:disabled="isBusy"
						@click="onRemove(close)"
					>
						<AppIcon name="trash" :size="15" />
						<span>{{ confirming ? "Confirmer la suppression" : "Supprimer" }}</span>
					</button>
				</template>
			</ActionMenu>
		</div>

		<!-- Barre de progression sous la carte quand le transfert est actif. -->
		<div v-if="isActive || isPaused" class="mt-2 h-1 overflow-hidden rounded-full bg-white/10">
			<div class="h-full bg-brand-gradient transition-normal" :style="{ width: `${props.entry.progress}%` }"></div>
		</div>
	</li>
</template>
