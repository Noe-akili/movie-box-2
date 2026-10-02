<script setup>
import PosterCard from "./PosterCard.vue";

const props = defineProps({
	items: { type: Array, default: () => [] },
	columns: { type: String, default: "grid-cols-3 sm:grid-cols-4 md:grid-cols-5 lg:grid-cols-6 xl:grid-cols-7" },
});
</script>

<template>
	<div class="grid gap-x-3 gap-y-5 md:gap-x-4" :class="props.columns">
		<PosterCard
			v-for="item in props.items"
			:key="item.id"
			:item="item"
			width="w-full"
		/>
	</div>
</template>
