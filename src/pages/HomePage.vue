<script setup>
import { onMounted, ref, watch } from "vue";
import { fetchHome } from "../api/client";
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

const { history } = useLibrary();
const settings = useSettings();

const load = async () => {
	isLoading.value = true;
	errorMessage.value = null;

	try {
		const data = await fetchHome();
		hero.value = data.hero || [];
		rails.value = data.rails || [];
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
		<div v-else class="skeleton h-[62vh] min-h-[400px] w-full rounded-none"></div>

		<div class="mx-auto max-w-[1800px] space-y-8 pt-6 md:space-y-10 md:pt-8">
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
				<MediaRow
					v-for="(rail, index) in rails"
					:key="rail.id"
					:title="rail.title"
					:items="rail.items"
					:href="rail.href"
					:ranked="index === 0"
				/>

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
