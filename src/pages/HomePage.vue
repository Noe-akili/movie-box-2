<script setup>
import { computed, onMounted, ref, watch } from "vue";
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

const load = async () => {
	isLoading.value = true;
	errorMessage.value = null;

	try {
		const data = await fetchHome();
		hero.value = data.hero || [];
		rails.value = data.rails || [];
		visibleRails.value = RAILS_STEP;
		isPartial.value = Boolean(data.partial);
	} catch (error) {
		errorMessage.value = error.message;
	} finally {
		isLoading.value = false;
	}
};

onMounted(load);
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
