<script setup>
import { computed, ref } from "vue";
import { downloadFileUrl } from "../api/client";
import { useDownloads } from "../stores/downloads";
import { toast } from "../stores/toast";
import { formatDate, formatEta, formatSize, formatSpeed } from "../util/format";
import AppIcon from "./AppIcon.vue";

/** Ligne d'un telechargement : progression en direct, lecture locale et enregistrement. */
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
		props.entry.format,
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

const onRemove = async () => {
	if (!confirming.value) {
		confirming.value = true;
		window.setTimeout(() => {
			confirming.value = false;
		}, 4000);
		return;
	}

	isBusy.value = true;
	try {
		await remove(props.entry.id);
		toast.info(`« ${heading.value} » supprimé du stockage`);
	} catch (error) {
		toast.error(error.message);
	} finally {
		isBusy.value = false;
		confirming.value = false;
	}
};
</script>

<template>
	<li class="surface overflow-hidden p-3 transition-normal hover:border-white/15">
		<div class="flex gap-3">
			<RouterLink
				:to="isDone && props.entry.playable ? { name: 'watch-local', params: { id: props.entry.id } } : '/downloads'"
				class="relative w-20 shrink-0 self-start overflow-hidden rounded-lg bg-inkCard ring-1 ring-white/5 sm:w-24"
			>
				<img
					v-if="props.entry.poster"
					:src="props.entry.poster"
					:alt="heading"
					loading="lazy"
					class="aspect-video w-full object-cover"
				/>
				<span v-else class="grid aspect-video w-full place-items-center text-white/30">
					<AppIcon name="download" :size="18" />
				</span>

				<span
					v-if="isActive || isPaused"
					class="absolute inset-x-1 bottom-1 h-1 overflow-hidden rounded-full bg-black/60"
				>
					<span class="block h-full bg-brand-gradient" :style="{ width: `${props.entry.progress}%` }"></span>
				</span>
			</RouterLink>

			<div class="min-w-0 flex-1 space-y-1.5">
				<div class="flex flex-wrap items-center gap-2">
					<span :class="badgeClass">{{ statusLabel }}</span>
					<span v-if="isDone" class="text-[11px] text-white/40">100 %</span>
					<span v-else-if="isActive" class="text-xs font-semibold text-brandSoft">{{ props.entry.progress }} %</span>
				</div>

				<h3 class="line-clamp-1 text-sm font-semibold text-white/90">{{ heading }}</h3>
				<p v-if="subheading" class="line-clamp-1 text-[11px] text-white/45">{{ subheading }}</p>
				<p class="line-clamp-1 text-[11px] text-white/40">{{ meta }}</p>

				<p v-if="liveLabel" class="text-[11px] text-white/55">{{ liveLabel }}</p>
				<p v-if="isDone && props.entry.completedAt" class="text-[11px] text-emerald-400/80">
					Stocké sur le serveur le {{ formatDate(props.entry.completedAt) }}
				</p>
				<p v-if="isFailed && props.entry.error" class="text-[11px] text-amber-300/90">{{ props.entry.error }}</p>
				<p v-if="isDone && !props.entry.playable" class="text-[11px] text-white/35">
					Format {{ props.entry.fileName.split(".").pop().toUpperCase() }} : à lire dans un lecteur externe après
					enregistrement sur l'appareil.
				</p>

				<div class="flex flex-wrap items-center gap-2 pt-1">
					<template v-if="isDone">
						<RouterLink
							v-if="props.entry.playable"
							:to="{ name: 'watch-local', params: { id: props.entry.id } }"
							class="btn btn-inverse px-3 py-2 text-xs"
						>
							<AppIcon name="play" :size="14" filled />
							<span>Regarder</span>
						</RouterLink>

						<a
							:href="downloadFileUrl(props.entry.id)"
							:download="props.entry.fileName"
							class="btn btn-inverse px-3 py-2 text-xs"
						>
							<AppIcon name="download" :size="14" />
							<span>Sur l'appareil</span>
						</a>
					</template>

					<button
						v-else-if="isActive"
						type="button"
						class="btn btn-inverse px-3 py-2 text-xs"
						:disabled="isBusy"
						@click="onPause"
					>
						<AppIcon name="pause" :size="14" />
						<span>Pause</span>
					</button>

					<button v-else type="button" class="btn btn-inverse px-3 py-2 text-xs" :disabled="isBusy" @click="onRetry">
						<AppIcon name="download" :size="14" />
						<span>Reprendre</span>
					</button>

					<button
						type="button"
						class="btn px-3 py-2 text-xs"
						:class="confirming ? 'btn-solid' : 'btn-ghost'"
						:disabled="isBusy"
						@click="onRemove"
					>
						<AppIcon name="trash" :size="14" />
						<span>{{ confirming ? "Confirmer" : "Supprimer" }}</span>
					</button>
				</div>
			</div>
		</div>
	</li>
</template>

