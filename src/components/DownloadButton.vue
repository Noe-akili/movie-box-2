<script setup>
import { computed, ref } from "vue";
import { downloadFileUrl } from "../api/client";
import { useDownloads } from "../stores/downloads";
import { toast } from "../stores/toast";
import { downloadFileName, downloadSourceId } from "../util/downloads";
import { formatSize } from "../util/format";
import ActionMenu from "./ActionMenu.vue";
import AppIcon from "./AppIcon.vue";
import DownloadDialog from "./DownloadDialog.vue";

/**
 * Bouton de telechargement d'un film ou d'un episode (fichier libre de droits).
 * Il suit l'etat reel du transfert cote backend et ouvre l'assistant (qualite / saison / episodes).
 */
const props = defineProps({
	item: { type: Object, required: true },
	stream: { type: Object, default: null },
	episode: { type: Object, default: null },
	label: { type: String, default: "Télécharger" },
	compact: { type: Boolean, default: false },
});

const { entryFor, retry } = useDownloads();
const isStarting = ref(false);
const isOpen = ref(false);

const sourceId = computed(() => downloadSourceId(props.item, props.episode));
const fileName = computed(() => downloadFileName(props.item, props.stream, { episode: props.episode }));
const entry = computed(() => (sourceId.value ? entryFor(sourceId.value, fileName.value) : null));
const isActive = computed(() => ["queued", "downloading"].includes(entry.value?.status));
const isDone = computed(() => entry.value?.status === "completed");
const isFailed = computed(
	() => Boolean(entry.value) && ["error", "paused", "canceled"].includes(entry.value.status),
);
const sizeLabel = computed(() => formatSize(props.stream?.size));
const iconSize = computed(() => (props.compact ? 14 : 18));

const onRetry = async () => {
	if (!entry.value) return;
	isStarting.value = true;
	try {
		await retry(entry.value.id);
		toast.success(`Reprise du téléchargement — ${entry.value.title}`);
	} catch (error) {
		toast.error(error.message);
	} finally {
		isStarting.value = false;
	}
};
</script>

<template>
	<RouterLink
		v-if="isActive"
		to="/downloads"
		class="btn btn-inverse relative overflow-hidden"
		:class="props.compact ? 'px-3 py-2 text-xs' : 'px-4 py-3'"
		:aria-label="`Téléchargement en cours : ${entry.progress} %`"
	>
		<span class="absolute inset-y-0 left-0 bg-brand/30 transition-normal" :style="{ width: `${entry.progress}%` }"></span>
		<AppIcon name="download" :size="iconSize" class="relative" />
		<span class="relative">{{ entry.progress }} %</span>
	</RouterLink>

	<template v-else-if="isDone">
		<RouterLink
			:to="{ name: 'watch-local', params: { id: entry.id } }"
			class="btn btn-inverse"
			:class="props.compact ? 'px-3 py-2 text-xs' : 'px-4 py-3'"
		>
			<AppIcon name="play" :size="iconSize" filled />
			<span>Regarder</span>
		</RouterLink>

		<ActionMenu :label="`Actions pour ${entry.title}`" :width="230">
			<a :href="downloadFileUrl(entry.id)" :download="entry.fileName" class="menu-item">
				<AppIcon name="hardDrive" :size="15" />
				<span>Enregistrer sur l'appareil</span>
			</a>
		</ActionMenu>
	</template>

	<button
		v-else-if="isFailed"
		type="button"
		class="btn btn-inverse text-amber-300"
		:class="props.compact ? 'px-3 py-2 text-xs' : 'px-4 py-3'"
		:disabled="isStarting"
		@click="onRetry"
	>
		<AppIcon name="download" :size="iconSize" />
		<span>{{ isStarting ? "Reprise…" : "Reprendre" }}</span>
	</button>

	<button
		v-else
		type="button"
		class="btn btn-inverse"
		:class="props.compact ? 'px-3 py-2 text-xs' : 'px-4 py-3'"
		@click="isOpen = true"
	>
		<AppIcon name="download" :size="iconSize" />
		<span>
			{{ props.label }}
			<span v-if="sizeLabel && !props.compact" class="text-white/45">· {{ sizeLabel }}</span>
		</span>
	</button>

	<DownloadDialog
		v-if="isOpen"
		:item="props.item"
		:stream="props.stream"
		:episode="props.episode"
		:label="props.label"
		@close="isOpen = false"
	/>
</template>
