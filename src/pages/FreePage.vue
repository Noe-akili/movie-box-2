<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { fetchFreeTitles } from "../api/client";
import { useInfiniteScroll } from "../composables/useInfiniteScroll";
import AppIcon from "../components/AppIcon.vue";
import EmptyState from "../components/EmptyState.vue";
import MediaGrid from "../components/MediaGrid.vue";
import SkeletonRail from "../components/SkeletonRail.vue";

const router = useRouter();

const FALLBACK_COLLECTIONS = [
	{ id: "home", label: "Accueil" },
	{ id: "movies", label: "Films" },
	{ id: "anime", label: "Anime" },
	{ id: "trending", label: "Tendances" },
	{ id: "nollywood", label: "Nollywood" },
	{ id: "asian", label: "Asie" },
	{ id: "western", label: "Hollywood" },
	{ id: "kids", label: "Jeunesse" },
];

const SORTS = [
	{ value: "popular", label: "Les plus vus" },
	{ value: "recent", label: "Récents" },
	{ value: "title", label: "A → Z" },
];

const source = ref(String(router.currentRoute.value.query.source || "internal"));
const collection = ref(String(router.currentRoute.value.query.collection || "home"));
const sort = ref("popular");
const search = ref(String(router.currentRoute.value.query.q || ""));

const sources = ref([]);
const collections = ref(FALLBACK_COLLECTIONS);
const items = ref([]);
const page = ref(1);
const hasMore = ref(false);
const total = ref(0);
const isLoading = ref(true);
const isLoadingMore = ref(false);
const errorMessage = ref(null);

const activeSource = computed(
	() => sources.value.find((item) => item.id === source.value) || null,
);

const heading = computed(() => {
	if (source.value === "internal") return "Catalogue MovieBox";
	return activeSource.value?.label || "Catalogue gratuit";
});

const subtitle = computed(() => {
	const note = activeSource.value?.note;
	if (total.value) {
		return [`${total.value.toLocaleString("fr-FR")} titres`, note]
			.filter(Boolean)
			.join(" · ");
	}
	return note || "Lecture directe, sans compte et sans paiement.";
});

let debounce = null;

// Défilement infini : de nouveaux titres en arrivant en bas de la page.
const { sentinel } = useInfiniteScroll(() => load({ reset: false }), {
	enabled: computed(() => hasMore.value && !isLoading.value && !isLoadingMore.value),
});

const syncQuery = () => {
	router.replace({
		query: {
			...(source.value !== "internal" ? { source: source.value } : {}),
			...(collection.value !== "home" ? { collection: collection.value } : {}),
			...(search.value ? { q: search.value } : {}),
		},
	});
};

const load = async ({ reset }) => {
	if (reset) {
		isLoading.value = true;
		items.value = [];
		page.value = 1;
	} else {
		isLoadingMore.value = true;
	}

	try {
		const data = await fetchFreeTitles({
			page: reset ? 1 : page.value + 1,
			source: source.value,
			collection: collection.value,
			sort: sort.value,
			q: search.value || undefined,
			rows: 30,
		});

		items.value = reset ? data.items : [...items.value, ...data.items];
		page.value = data.page;
		hasMore.value = data.hasMore;
		total.value = data.total;
		collections.value = data.collections?.length ? data.collections : FALLBACK_COLLECTIONS;
		if (data.sources?.length) sources.value = data.sources;
		errorMessage.value = null;
	} catch (error) {
		errorMessage.value = error.message;
	} finally {
		isLoading.value = false;
		isLoadingMore.value = false;
	}
};

// Changer de source réinitialise la collection sur celle par défaut du fournisseur.
const DEFAULT_COLLECTIONS = {
	internal: "home",
	archive: "feature_films",
	commons: "Films in the public domain",
};

const onSourceChange = (value) => {
	if (source.value === value) return;
	source.value = value;
	collection.value = DEFAULT_COLLECTIONS[value] || "";
	search.value = "";
};

watch([source, collection, sort], () => {
	syncQuery();
	load({ reset: true });
});

const onSearchInput = () => {
	window.clearTimeout(debounce);
	debounce = window.setTimeout(() => {
		syncQuery();
		load({ reset: true });
	}, 450);
};

onMounted(() => load({ reset: true }));
</script>


<template>
	<div class="pb-nav">
		<header class="page-top space-y-3 border-b border-white/5 bg-inkSoft/60 px-4 pb-4 md:px-6 lg:px-8">
			<div class="mx-auto max-w-[1800px] space-y-3">
				<div class="flex items-center gap-3">
					<span class="grid h-10 w-10 shrink-0 place-items-center rounded-2xl bg-emerald-500/15 text-emerald-400">
						<AppIcon name="gift" :size="20" />
					</span>
					<div class="min-w-0">
						<h1 class="line-clamp-1 text-xl font-bold tracking-tight md:text-2xl">{{ heading }}</h1>
						<p class="line-clamp-1 text-xs text-white/45">{{ subtitle }}</p>
					</div>
				</div>

				<div class="flex items-center gap-2 rounded-xl border border-inkLine bg-inkCard px-3 py-2.5">
					<AppIcon name="search" :size="18" class="text-white/40" />
					<input
						v-model="search"
						type="search"
						placeholder="Rechercher dans le catalogue gratuit…"
						class="w-full bg-transparent text-sm text-white placeholder:text-white/35 focus:outline-none"
						@input="onSearchInput"
						@keyup.enter="load({ reset: true })"
					/>
					<button v-if="search" type="button" class="text-white/40 hover:text-white" aria-label="Effacer" @click="((search = ''), syncQuery(), load({ reset: true }))">
						<AppIcon name="close" :size="16" />
					</button>
				</div>

				<!-- Plusieurs fournisseurs de vidéos : MovieBox, Internet Archive, Commons. -->
				<div v-if="sources.length > 1" class="space-y-1.5">
					<p class="px-1 text-[10px] font-semibold uppercase tracking-wider text-white/35">Source</p>
					<div class="rail -mx-1 px-1">
						<button
							v-for="option in sources"
							:key="option.id"
							type="button"
							class="chip"
							:class="source === option.id ? 'chip-active' : ''"
							:title="option.note"
							@click="onSourceChange(option.id)"
						>
							{{ option.label }}
						</button>
					</div>
				</div>

				<div class="rail -mx-1 px-1">
					<button
						v-for="option in collections"
						:key="option.id"
						type="button"
						class="chip"
						:class="collection === option.id ? 'chip-active' : ''"
						@click="collection = option.id"
					>
						{{ option.label }}
					</button>
				</div>

				<div class="rail -mx-1 px-1">
					<button
						v-for="option in SORTS"
						:key="option.value"
						type="button"
						class="chip"
						:class="sort === option.value ? 'chip-active' : ''"
						@click="sort = option.value"
					>
						{{ option.label }}
					</button>
				</div>
			</div>
		</header>

		<section class="mx-auto max-w-[1800px] px-4 py-6 md:px-6 lg:px-8">
			<SkeletonRail v-if="isLoading" :count="14" variant="grid" :title="false" />

			<EmptyState
				v-else-if="errorMessage"
				icon="info"
				title="Catalogue indisponible"
				:description="errorMessage"
				action-label="Réessayer"
				@action="load({ reset: true })"
			/>

			<template v-else>
				<MediaGrid :items="items" />

				<EmptyState
					v-if="!items.length"
					icon="archive"
					title="Aucun film trouvé"
					description="Essaie un autre mot-clé ou une autre collection."
				/>

				<!-- Témoin du défilement infini. -->
				<div ref="sentinel" class="h-10"></div>

				<div v-if="isLoadingMore" class="flex justify-center py-6">
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
