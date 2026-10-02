<script setup>
import { computed, onMounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { fetchFreeTitles } from "../api/client";
import AppIcon from "../components/AppIcon.vue";
import EmptyState from "../components/EmptyState.vue";
import MediaGrid from "../components/MediaGrid.vue";
import SkeletonRail from "../components/SkeletonRail.vue";

const router = useRouter();

const FALLBACK_COLLECTIONS = [
	{ id: "feature_films", label: "Longs-métrages libres" },
	{ id: "moviesandfilms", label: "Films populaires du web" },
	{ id: "classic_tv", label: "Séries TV classiques" },
	{ id: "television", label: "Télévision & émissions" },
	{ id: "Film_Noir", label: "Cinéma noir" },
	{ id: "SciFi_Horror", label: "Science-fiction & horreur" },
	{ id: "silent_films", label: "Films muets" },
	{ id: "animationandcartoons", label: "Animation" },
	{ id: "classic_cartoons", label: "Dessins animés" },
	{ id: "short_films", label: "Courts-métrages" },
	{ id: "prelinger", label: "Archives Prelinger" },
	{ id: "opensource_movies", label: "Vidéos libres (open source)" },
];

// Filtres de droits tels que déclarés par Internet Archive : « free » est appliqué par défaut côté backend.
const FALLBACK_LICENSES = [
	{ id: "free", label: "Domaine public & Creative Commons" },
	{ id: "publicdomain", label: "Domaine public uniquement" },
	{ id: "cc", label: "Creative Commons (hors domaine public)" },
	{ id: "all", label: "Toutes les archives (droits non vérifiés)" },
];

const DEFAULT_LICENSE = "free";

const SORTS = [
	{ value: "popular", label: "Les plus vus" },
	{ value: "recent", label: "Récents" },
	{ value: "oldest", label: "Plus anciens" },
	{ value: "title", label: "A → Z" },
];

const collections = ref(FALLBACK_COLLECTIONS);
const licenses = ref(FALLBACK_LICENSES);
const collection = ref(String(router.currentRoute.value.query.collection || "feature_films"));
const license = ref(String(router.currentRoute.value.query.license || DEFAULT_LICENSE));
const sort = ref("popular");
const search = ref(String(router.currentRoute.value.query.q || ""));

const items = ref([]);
const page = ref(1);
const hasMore = ref(false);
const total = ref(0);
const isLoading = ref(true);
const isLoadingMore = ref(false);
const errorMessage = ref(null);

const activeLicenseLabel = computed(
	() => licenses.value.find((option) => option.id === license.value)?.label || "Droits déclarés par l'Archive",
);

const subtitle = computed(() =>
	total.value
		? `${total.value.toLocaleString("fr-FR")} titres — ${activeLicenseLabel.value} · lecture directe, sans compte et sans paiement`
		: "Catalogue gratuit : lecture directe, sans compte et sans paiement.",
);

let debounce = null;

const syncQuery = () => {
	router.replace({
		query: {
			...(collection.value !== "feature_films" ? { collection: collection.value } : {}),
			...(license.value !== DEFAULT_LICENSE ? { license: license.value } : {}),
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
			collection: collection.value,
			license: license.value,
			sort: sort.value,
			q: search.value || undefined,
			rows: 30,
		});

		items.value = reset ? data.items : [...items.value, ...data.items];
		page.value = data.page;
		hasMore.value = data.hasMore;
		total.value = data.total;
		collections.value = data.collections?.length ? data.collections : FALLBACK_COLLECTIONS;
		licenses.value = data.licenses?.length ? data.licenses : FALLBACK_LICENSES;
		errorMessage.value = null;
	} catch (error) {
		errorMessage.value = error.message;
	} finally {
		isLoading.value = false;
		isLoadingMore.value = false;
	}
};

watch([collection, sort, license], () => {
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
		<header class="space-y-4 border-b border-white/5 bg-inkSoft/60 px-4 pb-5 pt-20 md:px-6 md:pt-24 lg:px-8">
			<div class="mx-auto max-w-[1800px] space-y-4">
				<div class="flex flex-wrap items-center gap-3">
					<span class="grid h-11 w-11 place-items-center rounded-2xl bg-emerald-500/15 text-emerald-400">
						<AppIcon name="gift" :size="22" />
					</span>
					<div>
						<h1 class="text-2xl font-bold tracking-tight md:text-3xl">Films gratuits</h1>
						<p class="text-sm text-white/45">{{ subtitle }}</p>
					</div>
				</div>

				<p class="max-w-3xl rounded-xl border border-emerald-500/20 bg-emerald-500/5 px-3 py-2 text-xs text-emerald-200/80">
					<strong>100 % légal :</strong> ces films et séries viennent d'Internet Archive et sont filtrés sur les
					droits <em>déclarés à la source</em> (domaine public ou licence Creative Commons) — le filtre
					« Domaine public &amp; Creative Commons » est actif par défaut. Les œuvres encore protégées sont
					référencées ailleurs avec leurs bandes-annonces officielles et les plateformes légales, dont les offres
					gratuites avec publicité.
				</p>

				<div class="flex items-center gap-2 rounded-xl border border-inkLine bg-inkCard px-3 py-2.5">
					<AppIcon name="search" :size="18" class="text-white/40" />
					<input
						v-model="search"
						type="search"
						placeholder="Chercher dans le catalogue gratuit (ex. Chaplin, Dracula, 1955…)"
						class="w-full bg-transparent text-sm text-white placeholder:text-white/35 focus:outline-none"
						@input="onSearchInput"
						@keyup.enter="load({ reset: true })"
					/>
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

				<div class="space-y-2">
					<p class="text-[11px] font-semibold uppercase tracking-wider text-white/35">Droits</p>
					<div class="rail -mx-1 px-1">
						<button
							v-for="option in licenses"
							:key="option.id"
							type="button"
							class="chip"
							:class="license === option.id ? 'chip-active' : ''"
							@click="license = option.id"
						>
							{{ option.label }}
						</button>
					</div>
					<p v-if="license === 'all'" class="text-xs text-amber-200/70">
						Filtre de droits désactivé : ces archives déclarent elles-mêmes leurs droits. Reviens sur
						« Domaine public &amp; Creative Commons » pour ne voir que des œuvres libres.
					</p>
				</div>
			</div>
		</header>

		<section class="mx-auto max-w-[1800px] px-4 py-6 md:px-6 lg:px-8">
			<SkeletonRail v-if="isLoading" :count="14" variant="grid" :title="false" />

			<EmptyState
				v-else-if="errorMessage"
				icon="info"
				title="Catalogue libre indisponible"
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

				<div v-if="hasMore" class="flex justify-center py-6">
					<button
						type="button"
						class="btn btn-inverse px-6"
						:disabled="isLoadingMore"
						@click="load({ reset: false })"
					>
						{{ isLoadingMore ? "Chargement…" : "Charger plus de films" }}
					</button>
				</div>
			</template>
		</section>
	</div>
</template>
