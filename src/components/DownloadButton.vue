<script setup>
import { computed, ref } from "vue";
import { downloadFileUrl } from "../api/client";
import { useDownloads } from "../stores/downloads";
import { toast } from "../stores/toast";
import { formatSize } from "../util/format";
import AppIcon from "./AppIcon.vue";

/**
 * Bouton de telechargement d'un film ou d'un episode (fichier libre de droits).
 * Il suit l'etat reel du transfert cote backend : a lancer, en cours (avec %), deja stocke.
 */
const props = defineProps({
	item: { type: Object, required: true },
	stream: { type: Object, required: true },
	episode: { type: Object, default: null },
	label: { type: String, default: "Télécharger" },
	compact: { type: Boolean, default: false },
});

const { entryFor, start, retry } = useDownloads();
const isStarting = ref(false);

const sourceId = computed(() => props.item?.archiveId || props.item?.sourceId || null);
const entry = computed(() =>
	sourceId.value && props.stream?.name ? entryFor(sourceId.value, props.stream.name) : null,
);
const isActive = computed(() => ["queued", "downloading"].includes(entry.value?.status));
const isDone = computed(() => entry.value?.status === "completed");
const isFailed = computed(() => Boolean(entry.value) && ["error", "paused", "canceled"].includes(entry.value.status));
const sizeLabel = computed(() => formatSize(props.stream?.size));
const iconSize = computed(() => (props.compact ? 14 : 18));

const onStart = async () => {
	if (!sourceId.value || !props.stream?.name) return;

	isStarting.value = true;

	try {
		if (entry.value) {
			await retry(entry.value.id);
			toast.success(`Reprise du téléchargement — ${entry.value.title}`);
		} else {
			await start({
				sourceId: sourceId.value,
				fileName: props.stream.name,
				title: props.episode ? props.episode.name || props.episode.label : props.item.title,
				kind: props.episode ? "episode" : "movie",
				seriesTitle: props.episode ? props.item.title : null,
				seasonNumber: props.episode?.season ?? null,
				episodeNumber: props.episode?.number ?? null,
				episodeName: props.episode?.name ?? null,
				label: props.episode?.label ?? null,
				poster: props.item.poster,
				backdrop: props.item.backdrop,
				quality: props.stream.quality,
				format: props.stream.format,
			});
			toast.success(`Téléchargement lancé — ${props.episode ? props.episode.name : props.item.title}`);
		}
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

		<a
			:href="downloadFileUrl(entry.id)"
			:download="entry.fileName"
			class="btn btn-inverse"
			:class="props.compact ? 'px-2.5 py-2' : 'px-3.5 py-3'"
			:aria-label="`Enregistrer ${entry.fileName} sur l'appareil`"
			:title="'Enregistrer sur l\'appareil'"
		>
			<AppIcon name="download" :size="iconSize" />
			<span v-if="!props.compact">Sur l'appareil</span>
		</a>
	</template>

	<button
		v-else
		type="button"
		class="btn"
		:class="[
			props.compact ? 'px-3 py-2 text-xs' : 'px-4 py-3',
			isFailed ? 'btn-inverse text-amber-300' : 'btn-inverse',
		]"
		:disabled="isStarting"
		@click="onStart"
	>
		<AppIcon name="download" :size="iconSize" />
		<span>
			{{ isStarting ? "Démarrage…" : isFailed ? "Reprendre" : props.label }}
			<span v-if="sizeLabel && !props.compact" class="text-white/45">· {{ sizeLabel }}</span>
		</span>
	</button>
</template>
