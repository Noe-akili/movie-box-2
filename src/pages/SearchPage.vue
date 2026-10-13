<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { searchTitles } from "../api/client";
import { useInfiniteScroll } from "../composables/useInfiniteScroll";
import AppIcon from "../components/AppIcon.vue";
import EmptyState from "../components/EmptyState.vue";
import MediaGrid from "../components/MediaGrid.vue";
import MediaRow from "../components/MediaRow.vue";
import SkeletonRail from "../components/SkeletonRail.vue";
import { useRecentSearches } from "../stores/recentSearches";

const router = useRouter();
const { recent, push, remove, clear } = useRecentSearches();

const TABS = [
	{ value: "all", label: "Tout" },
	{ value: "movie", label: "Films" },
	{ value: "tv", label: "Séries" },
];

const query = ref("");
const activeTab = ref("all");
const results = ref([]);
const free = ref({ items: [], total: 0 });
const isLoading = ref(false);
const isLoadingMore = ref(false);
const errorMessage = ref(null);
const searched = ref(false);
const page = ref(1);
const hasMore = ref(false);

let debounce = null;

const run = async (term, { remember = false, next = false } = {}) => {
	const value = String(term || "").trim();
	if (value.length < 2) {
		results.value = [];
		free.value = { items: [], total: 0 };
		searched.value = false;
		hasMore.value = false;
		return;
	}

	// Premier chargement : squelette plein écran. Suite : indicateur discret.
	if (next) isLoadingMore.value = true;
	else isLoading.value = true;
	errorMessage.value = null;

	try {
		const nextPage = next ? page.value + 1 : 1;
		const data = await searchTitles({ q: value, type: activeTab.value, page: nextPage });
		const incoming = data.results || [];

		results.value = next ? [...results.value, ...incoming] : incoming;
		page.value = nextPage;
		hasMore.value = incoming.length > 0;

		if (!next) {
			free.value = data.free || { items: [], total: 0 };
			searched.value = true;
			if (remember) push(value);
			router.replace({ query: { q: value } });
		}
	} catch (error) {
		errorMessage.value = error.message;
	} finally {
		isLoading.value = false;
		isLoadingMore.value = false;
	}
};

// Défilement infini : la suite des résultats en arrivant en bas de la page.
const { sentinel } = useInfiniteScroll(() => run(query.value, { next: true }), {
	enabled: computed(
		() =>
			hasMore.value &&
			!isLoading.value &&
			!isLoadingMore.value &&
			searched.value &&
			query.value.trim().length >= 2,
	),
});

const onInput = () => {
	window.clearTimeout(debounce);
	debounce = window.setTimeout(() => run(query.value, { remember: true }), 420);
};

const useRecent = (term) => {
	query.value = term;
	run(term);
};

watch(activeTab, () => {
	if (query.value.trim().length >= 2) run(query.value);
});

onMounted(() => {
	const initial = String(router.currentRoute.value.query.q || "");
	if (initial) {
		query.value = initial;
		run(initial);
	}
});
</script>

<template>
	<div class="pb-nav">
		<header class="sticky top-14 z-30 border-b border-white/5 bg-ink/95 px-4 py-3 backdrop-blur-xl md:top-16 md:px-6 lg:px-8">
			<div class="mx-auto max-w-[1800px] space-y-3">
				<div class="flex items-center gap-2 rounded-xl border border-inkLine bg-inkCard px-3 py-2.5">
					<AppIcon name="search" :size="18" class="text-white/40" />
					<input
						v-model="query"
						type="search"
						placeholder="Rechercher un film, une série, un animé…"
						class="w-full bg-transparent text-sm text-white placeholder:text-white/35 focus:outline-none"
						@input="onInput"
						@keyup.enter="run(query, { remember: true })"
					/>
					<button v-if="query" type="button" class="text-white/40 hover:text-white" aria-label="Effacer" @click="((query = ''), run(''))">
						<AppIcon name="close" :size="16" />
					</button>
				</div>

				<div class="rail -mx-1 px-1">
					<button
						v-for="tab in TABS"
						:key="tab.value"
						type="button"
						class="chip"
						:class="activeTab === tab.value ? 'chip-active' : ''"
						@click="activeTab = tab.value"
					>
						{{ tab.label }}
					</button>
				</div>
			</div>
		</header>

		<section class="mx-auto max-w-[1800px] space-y-8 px-4 py-6 md:px-6 lg:px-8">
			<div v-if="recent.length && !searched" class="space-y-3">
				<div class="flex items-center justify-between">
					<h2 class="section-title">Recherches récentes</h2>
					<button type="button" class="text-xs text-white/40 hover:text-white" @click="clear">Effacer</button>
				</div>
				<div class="flex flex-wrap gap-2">
					<button
						v-for="term in recent"
						:key="term"
						type="button"
						class="chip inline-flex items-center gap-2"
						@click="useRecent(term)"
					>
						{{ term }}
						<span role="button" class="text-white/35 hover:text-brandSoft" @click.stop="remove(term)">
							<AppIcon name="close" :size="12" />
						</span>
					</button>
				</div>
			</div>

			<SkeletonRail v-if="isLoading" :count="14" variant="grid" :title="false" />

			<EmptyState
				v-else-if="errorMessage"
				icon="info"
				title="Recherche impossible"
				:description="errorMessage"
				action-label="Réessayer"
				@action="run(query)"
			/>

			<template v-else-if="searched">
				<div v-if="results.length">
					<h2 class="section-title mb-3">Films & séries ({{ results.length }})</h2>
					<MediaGrid :items="results" />

					<!-- Défilement infini : la suite des résultats en arrivant en bas. -->
					<div ref="sentinel" class="h-10"></div>
					<div v-if="isLoadingMore" class="flex justify-center py-4">
						<span class="inline-flex items-center gap-2 text-xs text-white/45">
							<span class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-white/20 border-t-brandSoft"></span>
							Recherche de plus de résultats…
						</span>
					</div>
				</div>

				<MediaRow
					v-if="free.items.length"
					title="À voir gratuitement"
					subtitle="Films libres de droits correspondant à la recherche"
					:href="{ path: '/free', query: { q: query } }"
					:items="free.items"
				/>

				<EmptyState
					v-if="!results.length && !free.items.length"
					icon="search"
					title="Aucun résultat"
					:description="`Rien ne correspond à « ${query} ». Essaie un autre titre, un acteur ou une année.`"
				/>
			</template>

			<EmptyState
				v-else
				icon="search"
				title="Que veux-tu regarder ?"
				description="Cherche parmi plus d'un million de films et séries, et découvre les films gratuits du catalogue libre."
			/>
		</section>
	</div>
</template>
