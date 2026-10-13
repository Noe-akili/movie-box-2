<script setup>
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import AppIcon from "./AppIcon.vue";
import { detailRoute, isFree, isSeriesTitle, mediaTypeLabel, seasonCountOf, watchRoute } from "../util/media";
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

	// Type réel (film / série) + nombre de saisons : `seasons` peut être un compte
	// (TMDB) ou le tableau des saisons, d'où le passage par seasonCountOf.
	const series = isSeriesTitle(item);
	const seasonCount = seasonCountOf(item);

	return [
		isFree(item) ? "GRATUIT" : mediaTypeLabel(item),
		item.year,
		series && seasonCount ? `${seasonCount} saison${seasonCount > 1 ? "s" : ""}` : formatRuntime(item.runtime),
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
		class="group relative isolate h-[78vh] max-h-[760px] min-h-[520px] w-full overflow-hidden bg-inkDeep md:h-[86vh]"
		@touchstart.passive="onTouchStart"
		@touchend.passive="onTouchEnd"
	>
		<div
			v-for="(item, itemIndex) in props.items"
			:key="item.id"
			class="absolute inset-0 transition-opacity duration-700 ease-out"
			:class="itemIndex === index ? 'opacity-100' : 'opacity-0'"
		>
			<div
				class="h-full w-full bg-cover bg-center"
				:class="itemIndex === index ? 'animate-kenburns' : ''"
				:style="item.backdrop ? { backgroundImage: `url(${item.backdrop})` } : {}"
			></div>
		</div>

		<div class="absolute inset-0 bg-hero-gradient"></div>
		<div class="absolute inset-0 bg-hero-scrim"></div>

		<button
			v-if="props.items.length > 1"
			type="button"
			class="icon-btn absolute left-3 top-1/2 hidden -translate-y-1/2 opacity-0 transition-opacity duration-300 group-hover:opacity-100 md:grid lg:left-5"
			aria-label="Contenu précédent"
			@click="goTo((index - 1 + props.items.length) % props.items.length)"
		>
			<AppIcon name="chevronLeft" :size="22" />
		</button>
		<button
			v-if="props.items.length > 1"
			type="button"
			class="icon-btn absolute right-3 top-1/2 hidden -translate-y-1/2 opacity-0 transition-opacity duration-300 group-hover:opacity-100 md:grid lg:right-5"
			aria-label="Contenu suivant"
			@click="goTo((index + 1) % props.items.length)"
		>
			<AppIcon name="chevronRight" :size="22" />
		</button>

		<div v-if="active" class="absolute inset-x-0 bottom-0 px-4 pb-20 md:px-8 md:pb-24 lg:px-12">
			<div :key="active.id" class="max-w-2xl animate-riseIn space-y-3.5">
				<div class="flex items-center gap-2 text-[12px] font-bold uppercase tracking-[0.22em] text-brandSoft">
					<span class="h-4 w-1 rounded-full bg-brand-gradient"></span>
					{{ activeFree ? "Gratuit & légal" : "À l'affiche" }}
				</div>

				<h1 class="text-shadow-hero text-3xl font-black leading-[1.05] tracking-tight text-white sm:text-5xl lg:text-6xl">
					{{ active.title }}
				</h1>

				<div class="flex flex-wrap items-center gap-2">
					<span
						v-for="entry in meta"
						:key="entry"
						class="badge bg-white/10 text-white/80 backdrop-blur"
						:class="entry === 'GRATUIT' ? 'badge-free' : String(entry).startsWith('★') ? 'text-gold' : ''"
					>
						{{ entry }}
					</span>
				</div>

				<p v-if="active.overview" class="line-clamp-3 max-w-xl text-sm text-white/70 sm:text-base">
					{{ active.overview }}
				</p>

				<div class="flex flex-wrap items-center gap-2.5 pt-1.5">
					<RouterLink :to="watchRoute(active)" class="btn btn-solid px-6 py-3 text-[15px]">
						<AppIcon name="play" :size="19" filled />
						<span>{{ activeFree ? "Regarder gratuitement" : "Où regarder" }}</span>
					</RouterLink>

					<RouterLink :to="detailRoute(active)" class="btn btn-inverse px-5 py-3 text-[15px]">
						<AppIcon name="info" :size="19" />
						<span>Plus d'infos</span>
					</RouterLink>

					<button
						type="button"
						class="icon-btn h-11 w-11"
						:aria-label="activeSaved ? 'Retirer de Ma liste' : 'Ajouter à Ma liste'"
						@click="onToggleFavorite"
					>
						<AppIcon name="heart" :size="19" :filled="activeSaved" />
					</button>
				</div>
			</div>
		</div>

		<div
			v-if="props.items.length > 1"
			class="absolute bottom-6 left-1/2 z-10 flex -translate-x-1/2 items-center gap-1.5"
		>
			<button
				v-for="(item, itemIndex) in props.items"
				:key="`dot-${item.id}`"
				type="button"
				class="h-1.5 rounded-full transition-all duration-300"
				:class="itemIndex === index ? 'w-7 bg-brandSoft shadow-glowSoft' : 'w-1.5 bg-white/35 hover:bg-white/60'"
				:aria-label="`Afficher ${item.title}`"
				@click="goTo(itemIndex)"
			></button>
		</div>
	</section>
</template>
