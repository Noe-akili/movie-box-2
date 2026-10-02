<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import AppIcon from "./AppIcon.vue";
import { detailRoute, isFree, watchRoute } from "../util/media";
import { formatRuntime } from "../util/format";
import { useLibrary } from "../stores/library";
import { toast } from "../stores/toast";

const props = defineProps({
	items: { type: Array, default: () => [] },
});

const { isFavorite, toggleFavorite } = useLibrary();
const index = ref(0);
const active = computed(() => props.items[index.value] || null);
const activeFree = computed(() => isFree(active.value));
const activeSaved = computed(() => (active.value ? isFavorite(active.value) : false));

const meta = computed(() => {
	const item = active.value;
	if (!item) return [];

	return [
		item.mediaType === "archive" ? "GRATUIT" : item.mediaType === "tv" ? "SÉRIE" : "FILM",
		item.year,
		item.mediaType === "tv" && item.seasons ? `${item.seasons} saison${item.seasons > 1 ? "s" : ""}` : formatRuntime(item.runtime),
		item.rating ? `★ ${item.rating.toFixed(1)}` : null,
	].filter(Boolean);
});

let timer = null;
const stop = () => {
	if (timer) clearInterval(timer);
	timer = null;
};

const next = () => {
	if (props.items.length < 2) return;
	index.value = (index.value + 1) % props.items.length;
};

const start = () => {
	stop();
	if (props.items.length > 1) timer = setInterval(next, 8000);
};

const goTo = (target) => {
	index.value = target;
	start();
};

const onToggleFavorite = () => {
	if (!active.value) return;
	const added = toggleFavorite(active.value);
	if (added) toast.success(`« ${active.value.title} » ajouté à Ma liste`);
	else toast.info(`« ${active.value.title} » retiré de Ma liste`);
};

let touchStartX = null;
const onTouchStart = (event) => {
	touchStartX = event.touches[0].clientX;
	stop();
};

const onTouchEnd = (event) => {
	if (touchStartX === null) return;

	const delta = event.changedTouches[0].clientX - touchStartX;
	if (Math.abs(delta) > 45 && props.items.length > 1) {
		index.value = delta < 0
			? (index.value + 1) % props.items.length
			: (index.value - 1 + props.items.length) % props.items.length;
	}

	touchStartX = null;
	start();
};

watch(
	() => props.items.length,
	() => {
		index.value = 0;
		start();
	},
);

onMounted(start);
onBeforeUnmount(stop);
</script>

<template>
	<section
		v-if="props.items.length"
		class="relative isolate h-[72vh] max-h-[620px] min-h-[440px] w-full overflow-hidden bg-ink md:h-[78vh]"
		@touchstart.passive="onTouchStart"
		@touchend.passive="onTouchEnd"
	>
		<div
			v-for="(item, itemIndex) in props.items"
			:key="item.id"
			class="absolute inset-0 bg-cover bg-center transition-opacity duration-700"
			:class="itemIndex === index ? 'opacity-100' : 'opacity-0'"
			:style="item.backdrop ? { backgroundImage: `url(${item.backdrop})` } : {}"
		></div>

		<div class="absolute inset-0 bg-hero-scrim"></div>

		<div v-if="active" class="absolute inset-x-0 bottom-0 px-4 pb-16 md:px-6 md:pb-14 lg:px-8">
			<div class="max-w-2xl animate-fadeUp space-y-3">
				<div class="flex flex-wrap items-center gap-2">
					<span
						v-for="entry in meta"
						:key="entry"
						class="badge bg-black/50 text-white/80 backdrop-blur"
						:class="entry === 'GRATUIT' ? 'badge-free' : ''"
					>
						{{ entry }}
					</span>
				</div>

				<h1 class="text-2xl font-bold leading-tight tracking-tight text-white sm:text-4xl lg:text-5xl">
					{{ active.title }}
				</h1>

				<p v-if="active.overview" class="line-clamp-3 max-w-xl text-sm text-white/70 sm:text-base">
					{{ active.overview }}
				</p>

				<div class="flex flex-wrap items-center gap-2.5 pt-1">
					<RouterLink :to="watchRoute(active)" class="btn btn-solid px-6 py-3">
						<AppIcon name="play" :size="18" filled />
						<span>{{ activeFree ? "Regarder gratuitement" : "Où regarder" }}</span>
					</RouterLink>

					<RouterLink :to="detailRoute(active)" class="btn btn-inverse px-5 py-3">
						<AppIcon name="info" :size="18" />
						<span>Détails</span>
					</RouterLink>

					<button type="button" class="icon-btn" :aria-label="activeSaved ? 'Retirer de Ma liste' : 'Ajouter à Ma liste'" @click="onToggleFavorite">
						<AppIcon name="heart" :size="18" :filled="activeSaved" />
					</button>
				</div>
			</div>
		</div>

		<div v-if="props.items.length > 1" class="absolute bottom-5 left-1/2 z-10 flex -translate-x-1/2 items-center gap-1.5">
			<button
				v-for="(item, itemIndex) in props.items"
				:key="`dot-${item.id}`"
				type="button"
				class="h-1.5 rounded-full transition-all duration-300"
				:class="itemIndex === index ? 'w-6 bg-brandSoft' : 'w-1.5 bg-white/35 hover:bg-white/60'"
				:aria-label="`Afficher ${item.title}`"
				@click="goTo(itemIndex)"
			></button>
		</div>
	</section>
</template>
