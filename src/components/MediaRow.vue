<script setup>
import { ref } from "vue";
import AppIcon from "./AppIcon.vue";
import PosterCard from "./PosterCard.vue";

const props = defineProps({
	title: { type: String, default: "" },
	subtitle: { type: String, default: "" },
	items: { type: Array, default: () => [] },
	href: { type: [String, Object], default: null },
	ranked: { type: Boolean, default: false },
});

const scroller = ref(null);

const scrollBy = (direction) => {
	const element = scroller.value;
	if (!element) return;

	element.scrollBy({ left: direction * Math.max(260, element.clientWidth * 0.85), behavior: "smooth" });
};
</script>

<template>
	<section v-if="props.items.length" class="animate-fadeUp">
		<header class="mb-3 flex items-end justify-between gap-4 px-4 md:px-6 lg:px-8">
			<div>
				<h2 class="section-title">{{ props.title }}</h2>
				<p v-if="props.subtitle" class="text-xs text-white/40">{{ props.subtitle }}</p>
			</div>

			<div class="flex items-center gap-2">
				<RouterLink
					v-if="props.href"
					:to="props.href"
					class="hidden items-center gap-1 text-xs font-medium text-white/50 transition-normal hover:text-brandSoft sm:inline-flex"
				>
					Tout voir
					<AppIcon name="chevronRight" :size="14" />
				</RouterLink>
				<button
					type="button"
					class="icon-btn hidden h-8 w-8 sm:grid"
					aria-label="Faire défiler vers la gauche"
					@click="scrollBy(-1)"
				>
					<AppIcon name="chevronLeft" :size="16" />
				</button>
				<button
					type="button"
					class="icon-btn hidden h-8 w-8 sm:grid"
					aria-label="Faire défiler vers la droite"
					@click="scrollBy(1)"
				>
					<AppIcon name="chevronRight" :size="16" />
				</button>
			</div>
		</header>

		<div ref="scroller" class="rail px-4 md:px-6 lg:px-8">
			<div v-for="(item, index) in props.items" :key="item.id" class="rail-item">
				<PosterCard :item="item" :rank="props.ranked ? index + 1 : null" />
			</div>
		</div>
	</section>
</template>
