<script setup>
import { computed } from "vue";
import AppIcon from "./AppIcon.vue";
import { detailRoute, isFree, isSeriesTitle, mediaTypeLabel, seasonCountOf } from "../util/media";
import { formatRuntime } from "../util/format";
import { isLowPerformance } from "../util/perf";
import { useLibrary } from "../stores/library";
import { toast } from "../stores/toast";

const props = defineProps({
	item: { type: Object, required: true },
	rank: { type: [Number, String], default: null },
	width: { type: String, default: "w-[124px] sm:w-[142px] md:w-[156px] lg:w-[172px] xl:w-[188px]" },
	showMeta: { type: Boolean, default: true },
	// Indices de chargement d'image : `sizes` vient de la grille, `priority`
	// réserve le chargement immédiat aux premières cartes visibles.
	sizes: { type: String, default: "(max-width: 767px) 31vw, (max-width: 1199px) 18vw, 14vw" },
	priority: { type: Boolean, default: false },
});

/**
 * Sur un appareil faible, une seule vignette légère (w185) suffit : diviser la
 * résolution réduit fortement la mémoire de décodage de chaque poster.
 */
const imageSrc = computed(() => {
	const item = props.item;
	if (isLowPerformance.value) {
		return item.thumbSmall || item.thumb || item.poster || null;
	}
	return item.thumb || item.poster || null;
});

const imageSrcset = computed(() => {
	const item = props.item;
	const candidates = [
		[item.thumbSmall, "185w"],
		[item.thumb, "342w"],
		[item.poster, "500w"],
	].filter(([url]) => Boolean(url));

	// Évite les doublons quand une source renvoie la même URL à plusieurs tailles.
	const seen = new Set();
	return candidates
		.filter(([url]) => {
			if (seen.has(url)) return false;
			seen.add(url);
			return true;
		})
		.map(([url, descriptor]) => `${url} ${descriptor}`)
		.join(", ");
});

const { isFavorite, toggleFavorite } = useLibrary();

const saved = computed(() => isFavorite(props.item));
const free = computed(() => isFree(props.item));
// Type réel (film / série) déduit des données de l'API : les séries gratuites
// (archive ou source interne) ne portent pas toujours mediaType === "tv".
const series = computed(() => isSeriesTitle(props.item));
const seasonCount = computed(() => seasonCountOf(props.item));
const seriesLabel = computed(() => {
	if (!series.value) return null;
	return seasonCount.value ? `Série · ${seasonCount.value} saison${seasonCount.value > 1 ? "s" : ""}` : "Série";
});
const progress = computed(() => {
	const value = Number(props.item?.progress);
	if (!Number.isFinite(value) || value <= 0) return null;
	return Math.min(100, Math.round(value <= 1 ? value * 100 : value));
});
const meta = computed(() =>
	[
		props.item.year || null,
		series.value
			? seasonCount.value
				? `${seasonCount.value} saison${seasonCount.value > 1 ? "s" : ""}`
				: "Série"
			: formatRuntime(props.item.runtime),
	]
		.filter(Boolean)
		.join(" • "),
);

const initial = computed(() => (props.item.title || "?").trim().charAt(0).toUpperCase());

const onToggleFavorite = () => {
	const added = toggleFavorite(props.item);
	if (added) toast.success(`« ${props.item.title} » ajouté à Ma liste`);
	else toast.info(`« ${props.item.title} » retiré de Ma liste`);
};
</script>

<template>
	<RouterLink
		:to="detailRoute(props.item)"
		class="group flex h-full flex-col focus:outline-none"
		:class="props.width"
		:aria-label="`${props.item.title}${series ? ' — série' : ''}${free ? ' — gratuit' : ''}`"
	>
		<div
			class="relative overflow-hidden rounded-xl bg-inkCard shadow-poster ring-1 ring-white/5 transition-all duration-300 ease-out group-hover:-translate-y-1 group-hover:ring-white/20 group-hover:shadow-hover"
		>
			<img
				v-if="imageSrc"
				:src="imageSrc"
				:srcset="isLowPerformance ? undefined : imageSrcset || undefined"
				:sizes="!isLowPerformance && imageSrcset ? props.sizes : undefined"
				:alt="props.item.title"
				width="342"
				height="513"
				:loading="props.priority ? 'eager' : 'lazy'"
				:fetchpriority="props.priority ? 'high' : 'auto'"
				decoding="async"
				class="aspect-[2/3] w-full object-cover transition-transform duration-500 ease-out group-hover:scale-[1.06]"
			/>
			<div
				v-else
				class="grid aspect-[2/3] w-full place-items-center bg-gradient-to-br from-inkRaised to-ink text-2xl font-bold text-white/40"
			>
				{{ initial }}
			</div>

			<div
				class="pointer-events-none absolute inset-0 bg-gradient-to-t from-inkDeep/90 via-inkDeep/5 to-transparent opacity-90"
			></div>

			<span
				class="pointer-events-none absolute left-1/2 top-1/2 grid h-11 w-11 -translate-x-1/2 -translate-y-1/2 scale-75 place-items-center rounded-full bg-white/90 text-ink opacity-0 shadow-hover transition-all duration-300 ease-out group-hover:scale-100 group-hover:opacity-100"
			>
				<AppIcon name="play" :size="20" filled />
			</span>

			<span
				class="badge absolute left-1.5 top-1.5 shadow-soft"
				:class="
					free
						? 'badge-free'
						: series
							? 'bg-brand/25 text-brandSoft ring-1 ring-brand/40'
							: 'bg-black/60 text-white'
				"
			>
				{{ free ? "GRATUIT" : mediaTypeLabel(props.item) }}
			</span>
			<span
				v-if="seriesLabel"
				class="badge absolute left-1.5 top-8 bg-black/75 text-white/80"
			>
				{{ seriesLabel }}
			</span>

			<span
				v-if="props.item.rating"
				class="absolute bottom-1.5 left-1.5 inline-flex items-center gap-1 rounded-md bg-black/75 px-1.5 py-0.5 text-[11px] font-semibold text-gold"
			>
				<AppIcon name="star" :size="11" filled />
				{{ props.item.rating.toFixed(1) }}
			</span>

			<button
				type="button"
				class="absolute right-1.5 top-1.5 grid h-7 w-7 place-items-center rounded-full bg-black/65 text-white/90 transition-normal hover:bg-black/80"
				:class="saved ? 'text-brandSoft' : ''"
				:aria-label="saved ? `Retirer ${props.item.title} de Ma liste` : `Ajouter ${props.item.title} à Ma liste`"
				@click.prevent.stop="onToggleFavorite"
			>
				<AppIcon name="heart" :size="14" :filled="saved" />
			</button>

			<span
				v-if="props.rank"
				class="absolute -bottom-1 -left-1 text-4xl font-black italic leading-none text-white drop-shadow-[0_2px_8px_rgba(0,0,0,0.9)] sm:text-5xl"
			>
				{{ props.rank }}
			</span>

			<div v-if="progress" class="absolute inset-x-0 bottom-0 h-1 bg-white/15">
				<div class="h-full rounded-r-full bg-brand-gradient" :style="{ width: `${progress}%` }"></div>
			</div>
		</div>

		<div
			v-if="props.showMeta"
			class="mt-2 flex-1 space-y-0.5"
			:class="props.rank ? 'pl-8' : ''"
		>
			<h3 class="line-clamp-1 text-[13px] font-semibold text-white/90 transition-normal group-hover:text-brandSoft">
				{{ props.item.title }}
			</h3>
			<p v-if="meta" class="line-clamp-1 text-[11px] text-white/45">{{ meta }}</p>
		</div>
	</RouterLink>
</template>
