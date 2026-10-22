<script setup>
import { ref } from "vue";
import AppIcon from "./AppIcon.vue";
import PosterCard from "./PosterCard.vue";
import { mediaKey } from "../util/media";

const props = defineProps({
	title: { type: String, default: "" },
	subtitle: { type: String, default: "" },
	items: { type: Array, default: () => [] },
	href: { type: [String, Object], default: null },
	ranked: { type: Boolean, default: false },
});

const scroller = ref(null);

const keyFor = (item) => {
	const key = mediaKey(item) ?? item?.id ?? item?.title;
	return `${item?.source || item?.mediaType || "x"}:${key}`;
};

const scrollBy = (direction) => {
	const element = scroller.value;
	if (!element) return;

	element.scrollBy({ left: direction * Math.max(260, element.clientWidth * 0.85), behavior: "smooth" });
};
</script>

<template>
	<section v-if="props.items.length" class="animate-fadeUp">
		<header class="mb-3 flex items-end justify-between gap-4 px-4 md:px-6 lg:px-8">
			<div class="flex flex-wrap items-baseline gap-x-3 gap-y-0.5">
				<h2 class="section-title">{{ props.title }}</h2>
				<p v-if="props.subtitle" class="text-xs text-white/40">{{ props.subtitle }}</p>
			</div>

			<RouterLink
				v-if="props.href"
				:to="props.href"
				class="group/link inline-flex shrink-0 items-center gap-1 text-xs font-semibold text-white/50 transition-normal hover:text-brandSoft"
			>
				Tout voir
				<AppIcon name="chevronRight" :size="14" class="transition-transform duration-200 group-hover/link:translate-x-0.5" />
			</RouterLink>
		</header>

		<div class="group relative">
			<div ref="scroller" class="rail rail-fade px-4 md:px-6 lg:px-8">
				<div v-for="(item, index) in props.items" :key="keyFor(item)" class="rail-item">
					<PosterCard
						:item="item"
						:rank="props.ranked ? index + 1 : null"
						sizes="(max-width: 512px) 112px, (max-width: 767px) 128px, (max-width: 1199px) 142px, 160px"
					/>
				</div>
			</div>

			<button
				type="button"
				class="absolute inset-y-0 left-0 z-10 hidden w-14 items-center justify-start bg-gradient-to-r from-ink via-ink/70 to-transparent pl-1 opacity-0 transition-opacity duration-200 group-hover:opacity-100 md:flex"
				aria-label="Faire défiler vers la gauche"
				@click="scrollBy(-1)"
			>
				<span class="grid h-9 w-9 place-items-center rounded-full bg-inkRaised/90 text-white shadow-hover ring-1 ring-white/10 backdrop-blur transition-normal hover:scale-105 hover:bg-inkRaised">
					<AppIcon name="chevronLeft" :size="20" />
				</span>
			</button>
			<button
				type="button"
				class="absolute inset-y-0 right-0 z-10 hidden w-14 items-center justify-end bg-gradient-to-l from-ink via-ink/70 to-transparent pr-1 opacity-0 transition-opacity duration-200 group-hover:opacity-100 md:flex"
				aria-label="Faire défiler vers la droite"
				@click="scrollBy(1)"
			>
				<span class="grid h-9 w-9 place-items-center rounded-full bg-inkRaised/90 text-white shadow-hover ring-1 ring-white/10 backdrop-blur transition-normal hover:scale-105 hover:bg-inkRaised">
					<AppIcon name="chevronRight" :size="20" />
				</span>
			</button>
		</div>
	</section>
</template>
