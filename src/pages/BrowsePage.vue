<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { fetchDiscover, fetchGenres } from "../api/client";
import { cacheGet, cacheSet } from "../api/cache";
import { useInfiniteScroll } from "../composables/useInfiniteScroll";
import EmptyState from "../components/EmptyState.vue";
import MediaGrid from "../components/MediaGrid.vue";
import SkeletonRail from "../components/SkeletonRail.vue";
import { isLowPerformance } from "../util/perf";

const props = defineProps({
	mediaType: { type: String, default: "movie" },
});

const route = useRoute();
const router = useRouter();

const SORT_OPTIONS = {
	movie: [
		{ value: "popularity.desc", label: "Populaires" },
		{ value: "vote_average.desc", label: "Mieux notés" },
		{ value: "primary_release_date.desc", label: "Récents" },
		{ value: "revenue.desc", label: "Gros succès" },
	],
	tv: [
		{ value: "popularity.desc", label: "Populaires" },
		{ value: "vote_average.desc", label: "Mieux notées" },
		{ value: "first_air_date.desc", label: "Récentes" },
		{ value: "vote_count.desc", label: "Plus vues" },
	],
};

const LANGUAGES = [
	{ value: "", label: "Toutes langues" },
	{ value: "fr", label: "Français" },
	{ value: "en", label: "Anglais" },
	{ value: "es", label: "Espagnol" },
	{ value: "hi", label: "Hindi" },
	{ value: "ko", label: "Coréen" },
	{ value: "ja", label: "Japonais" },
];

const filters = ref({
	genre: String(route.query.genre || ""),
	sort: String(route.query.sort || "popularity.desc"),
	language: String(route.query.language || ""),
});

const items = ref([]);
const genres = ref([]);
const page = ref(1);
const hasMore = ref(false);
const isLoading = ref(true);
const isLoadingMore = ref(false);
const errorMessage = ref(null);

const sortOptions = computed(() => SORT_OPTIONS[props.mediaType] || SORT_OPTIONS.movie);
const pageTitle = computed(() => (props.mediaType === "tv" ? "Séries & Anime" : "Films"));
const pageSubtitle = computed(() =>
	props.mediaType === "tv"
		? "Toutes les séries du monde, filtrées par langue, genre et popularité"
		: "Tout le cinéma mondial : Hollywood, Bollywood, Nollywood, France, Corée…",
);

let controller = null;
let requestSeq = 0;

const itemKey = (item) =>
	`${item?.source || item?.mediaType || "x"}:${item?.id ?? item?.archiveId ?? item?.title}`;
const maxItems = () => (isLowPerformance.value ? 480 : 1200);

const mergeItems = (base, incoming) => {
	const seen = new Set(base.map(itemKey));
	const next = base.slice();
	for (const item of incoming) {
		const key = itemKey(item);
		if (seen.has(key)) continue;
		seen.add(key);
		next.push(item);
	}
	return next;
};

const load = async ({ reset }) => {
	const targetPage = reset ? 1 : page.value + 1;
	const key = `discover:${props.mediaType}:${filters.value.genre}:${filters.value.sort}:${filters.value.language}:${targetPage}`;

	controller?.abort();
	controller = new AbortController();
	const seq = ++requestSeq;
	const cached = cacheGet(key);

	if (reset) {
		isLoading.value = !cached;
		items.value = [];
		page.value = 1;
		if (cached) {
			items.value = cached.items || [];
			page.value = cached.page;
			hasMore.value = Boolean(cached.hasMore);
			errorMessage.value = null;
		}
	} else {
		isLoadingMore.value = true;
	}

	if (cached) {
		isLoading.value = false;
		isLoadingMore.value = false;
		return;
	}

	try {
		const data = await fetchDiscover(
			props.mediaType,
			{
				page: targetPage,
				genre: filters.value.genre || undefined,
				sort: filters.value.sort,
				language: filters.value.language || undefined,
			},
			{ signal: controller.signal },
		);

		if (seq !== requestSeq) return;
		cacheSet(key, data, 60_000);

		const nextItems = reset ? (data.items || []) : mergeItems(items.value, data.items || []);
		items.value = nextItems.slice(0, maxItems());
		page.value = data.page;
		hasMore.value = Boolean(data.hasMore) && nextItems.length < maxItems();
		errorMessage.value = null;
	} catch (error) {
		if (error.canceled || seq !== requestSeq) return;
		errorMessage.value = error.message;
	} finally {
		if (seq === requestSeq) {
			isLoading.value = false;
			isLoadingMore.value = false;
		}
	}
};

const loadGenres = async () => {
	try {
		const data = await fetchGenres(props.mediaType);
		genres.value = data.genres || [];
	} catch {
		genres.value = [];
	}
};

const toggleGenre = (id) => {
	const value = String(id);
	filters.value.genre = filters.value.genre === value ? "" : value;
};

watch(
	filters,
	() => {
		router.replace({
			query: {
				...(filters.value.genre ? { genre: filters.value.genre } : {}),
				...(filters.value.sort !== "popularity.desc" ? { sort: filters.value.sort } : {}),
				...(filters.value.language ? { language: filters.value.language } : {}),
			},
		});
		load({ reset: true });
	},
	{ deep: true },
);

watch(
	() => route.query,
	(query) => {
		const next = {
			genre: String(query.genre || ""),
			sort: String(query.sort || "popularity.desc"),
			language: String(query.language || ""),
		};

		if (
			next.genre !== filters.value.genre ||
			next.sort !== filters.value.sort ||
			next.language !== filters.value.language
		) {
			filters.value = next;
		}
	},
);

watch(
	() => props.mediaType,
	() => {
		genres.value = [];
		loadGenres();
	},
);

	// Défilement infini : la suite se charge en arrivant en bas de la page.
	const { sentinel } = useInfiniteScroll(() => load({ reset: false }), {
		enabled: computed(() => hasMore.value && !isLoading.value && !isLoadingMore.value),
	});

	onMounted(() => {
		load({ reset: true });
		loadGenres();
	});

	onBeforeUnmount(() => controller?.abort());
</script>


<template>
	<div class="pb-nav">
		<header class="page-top border-b border-white/5 bg-inkSoft/60 px-4 pb-5 md:px-6 lg:px-8">
			<div class="mx-auto max-w-[1800px] space-y-4">
				<div>
					<h1 class="text-2xl font-bold tracking-tight md:text-3xl">{{ pageTitle }}</h1>
					<p class="mt-1 text-sm text-white/45">{{ pageSubtitle }}</p>
				</div>

				<div class="rail -mx-1 px-1">
					<button
						v-for="option in sortOptions"
						:key="option.value"
						type="button"
						class="chip"
						:class="filters.sort === option.value ? 'chip-active' : ''"
						@click="filters.sort = option.value"
					>
						{{ option.label }}
					</button>
					<span class="mx-1 h-6 w-px self-center bg-white/10"></span>
					<button
						v-for="option in LANGUAGES"
						:key="`lang-${option.value}`"
						type="button"
						class="chip"
						:class="filters.language === option.value ? 'chip-active' : ''"
						@click="filters.language = option.value"
					>
						{{ option.label }}
					</button>
				</div>

				<div v-if="genres.length" class="rail -mx-1 px-1">
					<button
						v-for="genre in genres"
						:key="genre.id"
						type="button"
						class="chip"
						:class="filters.genre === String(genre.id) ? 'chip-active' : ''"
						@click="toggleGenre(genre.id)"
					>
						{{ genre.name }}
					</button>
				</div>
			</div>
		</header>

		<section class="mx-auto max-w-[1800px] px-4 py-6 md:px-6 lg:px-8">
			<SkeletonRail v-if="isLoading" :count="14" variant="grid" :title="false" />

			<EmptyState
				v-else-if="errorMessage"
				icon="info"
				title="Chargement impossible"
				:description="errorMessage"
				action-label="Réessayer"
				@action="load({ reset: true })"
			/>

			<template v-else>
				<MediaGrid :items="items" />

				<EmptyState
					v-if="!items.length"
					icon="filter"
					title="Aucun résultat avec ces filtres"
					description="Essaie une autre combinaison de genre, de langue ou de tri."
				/>

				<div ref="sentinel" class="h-10"></div>

				<!-- Indicateur de chargement (le scroll infini fait le reste). -->
				<div v-if="isLoadingMore" class="flex justify-center py-4">
					<span class="inline-flex items-center gap-2 text-xs text-white/45">
						<span class="h-3.5 w-3.5 animate-spin rounded-full border-2 border-white/20 border-t-brandSoft"></span>
						Chargement…
					</span>
				</div>

				<button
					v-else-if="hasMore"
					type="button"
					class="btn btn-inverse mx-auto flex px-6"
					@click="load({ reset: false })"
				>
					<span>Charger plus</span>
				</button>
			</template>
		</section>
	</div>
</template>
