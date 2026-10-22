<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { fetchHome } from "../api/client";
import AppIcon from "../components/AppIcon.vue";
import EmptyState from "../components/EmptyState.vue";
import HeroCarousel from "../components/HeroCarousel.vue";
import MediaRow from "../components/MediaRow.vue";
import SkeletonRail from "../components/SkeletonRail.vue";
import { useLibrary } from "../stores/library";
import { useSettings } from "../stores/settings";

const hero = ref([]);
const rails = ref([]);
const isLoading = ref(true);
const errorMessage = ref(null);
const isPartial = ref(false);
// L'accueil reste court : quelques rails visibles, le reste au clic sur « Voir plus ».
const RAILS_STEP = 4;
const visibleRails = ref(RAILS_STEP);
const shownRails = computed(() => rails.value.slice(0, visibleRails.value));
const hasMoreRails = computed(() => visibleRails.value < rails.value.length);

const { history } = useLibrary();
const settings = useSettings();

let controller = null;

const mergeRails = (base, incoming) => {
	const seen = new Set(base.map((rail) => rail.id));
	const next = base.slice();
	for (const rail of incoming) {
		if (!rail?.id || seen.has(rail.id)) continue;
		seen.add(rail.id);
		// Le catalogue interne reste le rail principal, comme avant le découpage.
		if (rail.id === "internal-home") next.unshift(rail);
		else next.push(rail);
	}
	return next;
};

/**
 * Accueil en deux temps : les rails rapides (TMDB) s'affichent d'abord, puis
 * les catalogues libres (Archive/Commons/Open/interne) arrivent en tâche de
 * fond. Sur un réseau lent, l'utilisateur ne fixe plus un écran vide.
 */
const load = async () => {
	controller?.abort();
	const local = new AbortController();
	controller = local;

	isLoading.value = true;
	errorMessage.value = null;
	isPartial.value = false;

	try {
		const data = await fetchHome({ part: "fast" }, { signal: local.signal });
		if (local.signal.aborted) return;
		hero.value = data.hero || [];
		rails.value = data.rails || [];
		visibleRails.value = RAILS_STEP;
		isPartial.value = Boolean(data.partial);
	} catch (error) {
		if (error.canceled || local.signal.aborted) return;
		errorMessage.value = error.message;
	} finally {
		if (!local.signal.aborted) isLoading.value = false;
	}

	// Les sources lentes ne bloquent jamais le premier rendu.
	if (!local.signal.aborted) loadExtra(local.signal);
};

const loadExtra = async (signal) => {
	try {
		const data = await fetchHome({ part: "extra" }, { signal });
		if (signal.aborted) return;
		rails.value = mergeRails(rails.value, data.rails || []);
		if (!hero.value.length && data.hero?.length) hero.value = data.hero;
		if (data.partial) isPartial.value = true;
	} catch (error) {
		if (!error.canceled) isPartial.value = true;
	}
};

onMounted(load);
onBeforeUnmount(() => controller?.abort());
watch(() => settings.value.language, load);
</script>

<template>
	<div class="pb-nav">
		<HeroCarousel v-if="hero.length" :items="hero" />
		<div v-else class="skeleton h-[58vh] min-h-[340px] w-full rounded-none"></div>

		<div class="mx-auto max-w-[1800px] space-y-5 pt-4 md:space-y-6 md:pt-5">
			<MediaRow
				v-if="history.length"
				title="Reprendre la lecture"
				subtitle="Votre historique récent"
				:items="history.slice(0, 14)"
			/>

			<EmptyState
				v-if="errorMessage"
				icon="info"
				title="Le catalogue ne répond pas"
				:description="errorMessage"
				action-label="Réessayer"
				@action="load"
			/>

			<template v-else-if="isLoading">
				<SkeletonRail v-for="index in 3" :key="`skeleton-${index}`" :count="7" />
			</template>

			<template v-else>
				<p
					v-if="isPartial && rails.length"
					class="flex flex-wrap items-center justify-between gap-3 rounded-xl bg-inkCard px-4 py-3 text-xs text-white/60 ring-1 ring-white/5"
				>
					<span>Certains contenus n'ont pas encore chargé (source un peu lente).</span>
					<button
						type="button"
						class="font-semibold text-brandSoft transition-normal hover:text-white"
						@click="load"
					>
						Recharger
					</button>
				</p>

				<MediaRow
					v-for="(rail, index) in shownRails"
					:key="rail.id"
					:title="rail.title"
					:items="rail.items"
					:href="rail.href"
					:ranked="index === 0"
				/>

				<button
					v-if="hasMoreRails"
					type="button"
					class="btn btn-inverse mx-auto flex px-6"
					@click="visibleRails += RAILS_STEP"
				>
					<AppIcon name="chevronDown" :size="16" />
					<span>Voir plus de catégories</span>
				</button>

				<EmptyState
					v-if="!rails.length"
					icon="film"
					title="Aucun contenu disponible"
					description="Vérifie la configuration du serveur (clé TMDB) puis réessaie."
					action-label="Recharger"
					@action="load"
				/>
			</template>
		</div>
	</div>
</template>
