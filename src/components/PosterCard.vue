<script setup>
import { computed } from "vue";
import AppIcon from "./AppIcon.vue";
import { detailRoute, isFree } from "../util/media";
import { formatRuntime } from "../util/format";
import { useLibrary } from "../stores/library";
import { toast } from "../stores/toast";

const props = defineProps({
	item: { type: Object, required: true },
	rank: { type: [Number, String], default: null },
	width: { type: String, default: "w-[124px] sm:w-[142px] md:w-[156px] lg:w-[172px] xl:w-[188px]" },
	showMeta: { type: Boolean, default: true },
});

const { isFavorite, toggleFavorite } = useLibrary();

const saved = computed(() => isFavorite(props.item));
const free = computed(() => isFree(props.item));
const progress = computed(() => {
	const value = Number(props.item?.progress);
	if (!Number.isFinite(value) || value <= 0) return null;
	return Math.min(100, Math.round(value <= 1 ? value * 100 : value));
});
const meta = computed(() =>
	[props.item.year || null, props.item.mediaType === "archive" ? null : formatRuntime(props.item.runtime)]
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
		class="group block focus:outline-none"
		:class="props.width"
	>
		<div
			class="relative overflow-hidden rounded-xl bg-inkCard shadow-poster ring-1 ring-white/5 transition-all duration-300 ease-out group-hover:-translate-y-1 group-hover:ring-white/20 group-hover:shadow-hover"
		>
			<img
				v-if="props.item.poster"
				:src="props.item.poster"
				:alt="props.item.title"
				loading="lazy"
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
				class="pointer-events-none absolute left-1/2 top-1/2 grid h-11 w-11 -translate-x-1/2 -translate-y-1/2 scale-75 place-items-center rounded-full bg-white/90 text-ink opacity-0 shadow-hover backdrop-blur transition-all duration-300 ease-out group-hover:scale-100 group-hover:opacity-100"
			>
				<AppIcon name="play" :size="20" filled />
			</span>

			<span
				class="badge absolute left-1.5 top-1.5 shadow-soft"
				:class="free ? 'badge-free' : 'bg-black/60 text-white backdrop-blur'"
			>
				{{ free ? "GRATUIT" : props.item.mediaType === "tv" ? "SÉRIE" : "FILM" }}
			</span>

			<span
				v-if="props.item.rating"
				class="absolute bottom-1.5 left-1.5 inline-flex items-center gap-1 rounded-md bg-black/65 px-1.5 py-0.5 text-[11px] font-semibold text-gold backdrop-blur"
			>
				<AppIcon name="star" :size="11" filled />
				{{ props.item.rating.toFixed(1) }}
			</span>

			<button
				type="button"
				class="absolute right-1.5 top-1.5 grid h-7 w-7 place-items-center rounded-full bg-black/55 text-white/90 backdrop-blur transition-normal hover:bg-black/80"
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

		<div v-if="props.showMeta" class="mt-2 space-y-0.5" :class="props.rank ? 'pl-8' : ''">
			<h3 class="line-clamp-1 text-[13px] font-semibold text-white/90 transition-normal group-hover:text-brandSoft">
				{{ props.item.title }}
			</h3>
			<p v-if="meta" class="line-clamp-1 text-[11px] text-white/45">{{ meta }}</p>
		</div>
	</RouterLink>
</template>
