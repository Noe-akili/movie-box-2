<script setup>
import { computed, onMounted, onUnmounted, ref } from "vue";
import { receivedTransferDownloadUrl } from "../api/client";
import AppIcon from "../components/AppIcon.vue";
import DownloadGroup from "../components/DownloadGroup.vue";
import DownloadRow from "../components/DownloadRow.vue";
import EmptyState from "../components/EmptyState.vue";
import { useDownloads } from "../stores/downloads";
import { useTransfer } from "../stores/transfer";
import { startTransferPolling, stopTransferPolling } from "../stores/transfer";
import { toast } from "../stores/toast";
import { formatDate, formatSize } from "../util/format";

/** Onglet « Téléchargements » : progression des transferts et lecture des fichiers stockes. */
const { summary, isLoading, lastError, pendingItems, completedItems, refresh, remove } = useDownloads();
const { received, removeReceived, clearReceived } = useTransfer();

const TABS = [
	{ value: "pending", label: "En cours" },
	{ value: "completed", label: "Téléchargés" },
	{ value: "received", label: "Reçus" },
];

const activeTab = ref("pending");
const confirming = ref(false);

const visible = computed(() => (activeTab.value === "pending" ? pendingItems.value : completedItems.value));

/**
 * Les épisodes d'une même série sont rangés dans un « dossier » (The Rookie →
 * un seul bloc contenant tous ses épisodes). Les films restent des cartes seules.
 */
const groups = computed(() => {
	const map = new Map();
	const singles = [];

	for (const entry of visible.value) {
		if (entry.kind === "episode" && entry.seriesTitle) {
			const key = entry.seriesTitle;
			if (!map.has(key)) map.set(key, { title: key, entries: [] });
			map.get(key).entries.push(entry);
		} else {
			singles.push(entry);
		}
	}

	const result = [];
	for (const group of map.values()) {
		// Ordre naturel : saison puis numéro d'épisode.
		group.entries.sort((a, b) => {
			const season = (a.seasonNumber || 0) - (b.seasonNumber || 0);
			if (season !== 0) return season;
			return (a.episodeNumber || 0) - (b.episodeNumber || 0);
		});
		result.push({ type: "series", ...group });
	}

	for (const entry of singles) result.push({ type: "single", entry });

	return result;
});

/** Supprime tous les épisodes d'un dossier (confirmation déjà gérée par l'enfant). */
const onRemoveMany = async (ids) => {
	try {
		await Promise.all(ids.map((id) => remove(id)));
		toast.info("Épisodes supprimés");
	} catch (error) {
		toast.error(error.message);
	}
};
const receivedBytes = computed(() =>
	received.value.reduce((sum, file) => sum + (Number(file.size) || 0), 0),
);
const usedBytes = computed(
	() => (summary.value.completedBytes || 0) + (summary.value.activeBytes || 0) + receivedBytes.value,
);
const usedLabel = computed(() => formatSize(usedBytes.value));
const badgeFor = (tab) => {
	if (tab === "received") return received.value.length;
	return tab === "pending" ? pendingItems.value.length : completedItems.value.length;
};

const iconFor = (file) => {
	const mime = String(file.mime || "");
	if (mime.startsWith("video/") || mime.startsWith("audio/")) return "film";
	if (mime.startsWith("image/")) return "image";
	return "archive";
};

const labelFor = (file) => {
	const mime = String(file.mime || "");
	if (mime.startsWith("video/")) return "Vidéo";
	if (mime.startsWith("audio/")) return "Audio";
	if (mime.startsWith("image/")) return "Image";
	if (mime === "application/pdf") return "PDF";
	return "Fichier";
};

const toneFor = (file) => {
	const mime = String(file.mime || "");
	if (mime.startsWith("video/")) return "bg-brand/15 text-brandSoft";
	if (mime.startsWith("audio/")) return "bg-violet-500/15 text-violet-300";
	if (mime.startsWith("image/")) return "bg-sky-500/15 text-sky-300";
	return "bg-white/5 text-white/50";
};

const onClearCompleted = async () => {
	if (!confirming.value) {
		confirming.value = true;
		window.setTimeout(() => {
			confirming.value = false;
		}, 4000);
		return;
	}

	confirming.value = false;

	try {
		await Promise.all(completedItems.value.map((entry) => remove(entry.id)));
		toast.info("Téléchargements supprimés");
	} catch (error) {
		toast.error(error.message);
	}
};

/** Supprime un fichier reçu (disque + entrée de la salle). */
const onRemoveReceived = async (id) => {
	try {
		await removeReceived(id);
		toast.info("Fichier reçu supprimé");
	} catch (error) {
		toast.error(error.message);
	}
};

const onClearReceived = async () => {
	if (!confirming.value) {
		confirming.value = true;
		window.setTimeout(() => {
			confirming.value = false;
		}, 4000);
		return;
	}

	confirming.value = false;
	try {
		await clearReceived();
		toast.info("Fichiers reçus supprimés");
	} catch (error) {
		toast.error(error.message);
	}
};

onMounted(() => {
	refresh();
	// Les fichiers reçus arrivent par la salle Wi-Fi : on rafraîchit la liste
	// pendant que l'écran est ouvert, comme sur la page Transferts.
	startTransferPolling();
});

onUnmounted(() => stopTransferPolling());
</script>


<template>
	<div class="pb-nav">
		<header class="page-top space-y-4 border-b border-white/5 bg-inkSoft/60 px-4 pb-5 md:px-6 lg:px-8">
			<div class="mx-auto flex max-w-[1800px] flex-wrap items-end justify-between gap-4">
				<div class="flex items-center gap-3">
					<span class="grid h-11 w-11 place-items-center rounded-2xl bg-brand/15 text-brandSoft">
						<AppIcon name="download" :size="22" />
					</span>
					<div>
						<h1 class="text-2xl font-bold tracking-tight md:text-3xl">Téléchargements</h1>
						<p class="mt-1 text-sm text-white/45">
							{{ summary.completed }} élément(s) enregistré(s)
							<span v-if="summary.active + summary.queued"> · {{ summary.active + summary.queued }} en cours</span>
							<span v-if="received.length"> · {{ received.length }} reçu{{ received.length > 1 ? "s" : "" }}</span>
							<span v-if="usedLabel"> · {{ usedLabel }} utilisés</span>
						</p>
					</div>
				</div>

				<button
					v-if="
						(activeTab === 'completed' && completedItems.length) ||
						(activeTab === 'received' && received.length)
					"
					type="button"
					class="btn px-3 py-2 text-xs"
					:class="confirming ? 'btn-solid' : 'btn-inverse'"
					@click="activeTab === 'received' ? onClearReceived() : onClearCompleted()"
				>
					<AppIcon name="trash" :size="14" />
					<span>{{
						confirming
							? "Confirmer la suppression"
							: activeTab === "received"
								? "Vider les reçus"
								: "Vider les téléchargés"
					}}</span>
				</button>
			</div>

			<div class="mx-auto max-w-[1800px]">
				<div class="rail -mx-1 px-1">
					<button
						v-for="tab in TABS"
						:key="tab.value"
						type="button"
						class="chip"
						:class="activeTab === tab.value ? 'chip-active' : ''"
						@click="((activeTab = tab.value), (confirming = false))"
					>
						{{ tab.label }} ({{ badgeFor(tab.value) }})
					</button>
				</div>
			</div>
		</header>

		<section class="mx-auto max-w-[1800px] space-y-4 px-4 py-6 md:px-6 lg:px-8">
			<p v-if="lastError" class="rounded-xl border border-amber-500/25 bg-amber-500/5 px-3 py-2 text-xs text-amber-200/80">
				{{ lastError }}
			</p>

			<!-- Fichiers reçus par la salle Wi-Fi : conservés sur l'appareil et lisibles ici. -->
			<ul v-if="activeTab === 'received' && received.length" class="space-y-2">
				<li
					v-for="file in received"
					:key="file.id"
					class="surface flex items-center gap-3 p-3"
				>
					<span class="grid h-11 w-11 shrink-0 place-items-center rounded-xl" :class="toneFor(file)">
						<AppIcon :name="iconFor(file)" :size="18" />
					</span>

					<div class="min-w-0 flex-1">
						<p class="truncate text-sm font-semibold text-white/90">{{ file.name }}</p>
						<p class="text-[11px] text-white/40">
							{{ labelFor(file) }} · {{ formatSize(file.size) }}
							<span v-if="file.addedAt"> · {{ formatDate(file.addedAt) }}</span>
							<span v-if="file.uploader"> · de {{ file.uploader }}</span>
						</p>
					</div>

					<div class="flex shrink-0 items-center gap-1.5">
						<RouterLink
							v-if="file.playable"
							:to="{ name: 'watch-received', params: { id: file.id } }"
							class="btn btn-solid px-3 py-2 text-xs"
						>
							<AppIcon name="play" :size="13" filled />
							<span>Lire</span>
						</RouterLink>

						<a
							:href="receivedTransferDownloadUrl(file.id)"
							:download="file.name"
							class="icon-btn h-9 w-9"
							title="Enregistrer sur l'appareil"
							aria-label="Enregistrer sur l'appareil"
						>
							<AppIcon name="hardDrive" :size="15" />
						</a>

						<button
							type="button"
							class="icon-btn h-9 w-9 text-white/50 hover:text-red-300"
							aria-label="Supprimer"
							@click="onRemoveReceived(file.id)"
						>
							<AppIcon name="trash" :size="15" />
						</button>
					</div>
				</li>
			</ul>

			<EmptyState
				v-else-if="activeTab === 'received'"
				icon="transfer"
				title="Aucun fichier reçu"
				description="Les fichiers envoyés depuis un autre appareil du Wi-Fi sont enregistrés ici : lecture directe ou enregistrement sur l'appareil."
				action-label="Ouvrir les transferts"
				@action="$router.push('/transfers')"
			/>

			<ul v-else-if="groups.length" class="space-y-3">
				<template v-for="group in groups" :key="group.type === 'series' ? `series-${group.title}` : group.entry.id">
					<DownloadGroup
						v-if="group.type === 'series'"
						:group="group"
						@remove-many="onRemoveMany"
					/>
					<DownloadRow v-else :entry="group.entry" />
				</template>
			</ul>

			<div v-else-if="isLoading" class="space-y-3">
				<div v-for="index in 3" :key="`sk-${index}`" class="skeleton h-24 w-full"></div>
			</div>

			<EmptyState
				v-else-if="activeTab === 'pending'"
				icon="download"
				title="Aucun téléchargement en cours"
				description="Depuis un film gratuit ou un épisode de série libre de droits, touche « Télécharger » : la progression s'affichera ici."
				action-label="Voir les titres gratuits"
				@action="$router.push('/free')"
			/>

			<EmptyState
				v-else
				icon="archive"
				title="Rien de téléchargé pour l'instant"
				description="Les films et épisodes entièrement téléchargés apparaissent ici : lecture hors ligne directe ou enregistrement sur l'appareil."
				action-label="Explorer le catalogue gratuit"
				@action="$router.push('/free')"
			/>
		</section>

		<!-- Bouton fixe : ouvre le partage de fichiers en Wi-Fi local. -->
		<RouterLink
			to="/transfers"
			class="transfer-fab fixed right-4 z-30 grid h-14 w-14 place-items-center rounded-full bg-brand-gradient text-white shadow-glow transition-normal hover:opacity-90 active:scale-95 lg:right-8"
			aria-label="Transferts et partage Wi-Fi"
			title="Transferts et partage Wi-Fi"
		>
			<AppIcon name="transfer" :size="24" />
		</RouterLink>
	</div>
</template>
