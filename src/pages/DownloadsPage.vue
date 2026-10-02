<script setup>
import { computed, onMounted, ref } from "vue";
import AppIcon from "../components/AppIcon.vue";
import DownloadRow from "../components/DownloadRow.vue";
import EmptyState from "../components/EmptyState.vue";
import { useDownloads } from "../stores/downloads";
import { toast } from "../stores/toast";
import { formatSize } from "../util/format";

/** Onglet « Téléchargements » : progression des transferts et lecture des fichiers stockes. */
const { summary, isLoading, lastError, pendingItems, completedItems, refresh, remove } = useDownloads();

const TABS = [
	{ value: "pending", label: "En cours" },
	{ value: "completed", label: "Téléchargés" },
];

const activeTab = ref("pending");
const confirming = ref(false);

const visible = computed(() => (activeTab.value === "pending" ? pendingItems.value : completedItems.value));
const usedBytes = computed(() => (summary.value.completedBytes || 0) + (summary.value.activeBytes || 0));
const usedLabel = computed(() => formatSize(usedBytes.value));
const freeLabel = computed(() => (summary.value.freeBytes ? formatSize(summary.value.freeBytes) : null));
const limitLabel = computed(() => (summary.value.maxBytes ? formatSize(summary.value.maxBytes) : null));
const badgeFor = (tab) => (tab === "pending" ? pendingItems.value.length : completedItems.value.length);

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
		toast.info("Téléchargements supprimés du stockage du serveur");
	} catch (error) {
		toast.error(error.message);
	}
};

onMounted(() => refresh());
</script>


<template>
	<div class="pb-nav">
		<header class="space-y-4 border-b border-white/5 bg-inkSoft/60 px-4 pb-5 pt-20 md:px-6 md:pt-24 lg:px-8">
			<div class="mx-auto flex max-w-[1800px] flex-wrap items-end justify-between gap-4">
				<div class="flex items-center gap-3">
					<span class="grid h-11 w-11 place-items-center rounded-2xl bg-brand/15 text-brandSoft">
						<AppIcon name="download" :size="22" />
					</span>
					<div>
						<h1 class="text-2xl font-bold tracking-tight md:text-3xl">Téléchargements</h1>
						<p class="mt-1 text-sm text-white/45">
							{{ summary.completed }} titre(s) stocké(s) sur le serveur
							<span v-if="summary.active + summary.queued"> · {{ summary.active + summary.queued }} en cours</span>
							<span v-if="usedLabel"> · {{ usedLabel }} utilisés</span>
						</p>
					</div>
				</div>

				<button
					v-if="activeTab === 'completed' && completedItems.length"
					type="button"
					class="btn px-3 py-2 text-xs"
					:class="confirming ? 'btn-solid' : 'btn-inverse'"
					@click="onClearCompleted"
				>
					<AppIcon name="trash" :size="14" />
					<span>{{ confirming ? "Confirmer la suppression" : "Vider les téléchargés" }}</span>
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
						@click="activeTab = tab.value"
					>
						{{ tab.label }} ({{ badgeFor(tab.value) }})
					</button>
				</div>
			</div>
		</header>

		<section class="mx-auto max-w-[1800px] space-y-4 px-4 py-6 md:px-6 lg:px-8">
			<p class="rounded-xl border border-emerald-500/20 bg-emerald-500/5 px-3 py-2 text-xs text-emerald-200/80">
				<strong>100 % légal :</strong> seuls les films et les épisodes de séries <em>libres de droits</em>
				(Internet Archive : domaine public ou diffusion autorisée) peuvent être téléchargés. Le backend les stocke
				sur son disque, tu peux ensuite les lire hors ligne ou les enregistrer sur ton appareil.
				<span v-if="freeLabel">Espace disque libre : {{ freeLabel }}.</span>
				<span v-if="limitLabel">Taille maximale par fichier : {{ limitLabel }}.</span>
			</p>

			<p v-if="lastError" class="rounded-xl border border-amber-500/25 bg-amber-500/5 px-3 py-2 text-xs text-amber-200/80">
				{{ lastError }}
			</p>

			<ul v-if="visible.length" class="space-y-3">
				<DownloadRow v-for="entry in visible" :key="entry.id" :entry="entry" />
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
	</div>
</template>
