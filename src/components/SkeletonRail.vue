<script setup>
const props = defineProps({
	count: { type: Number, default: 8 },
	variant: { type: String, default: "row" },
	title: { type: Boolean, default: true },
});
</script>

<template>
	<section class="animate-fadeUp" aria-hidden="true">
		<div v-if="props.title" class="mb-3 flex items-center justify-between px-4 md:px-6 lg:px-8">
			<div class="skeleton h-4 w-40"></div>
			<div class="skeleton hidden h-8 w-20 sm:block"></div>
		</div>

		<div v-if="props.variant === 'row'" class="rail px-4 md:px-6 lg:px-8">
			<div
				v-for="index in props.count"
				:key="index"
				class="rail-item w-[112px] sm:w-[128px] md:w-[136px] lg:w-[150px]"
			>
				<div class="skeleton aspect-[2/3] w-full"></div>
				<div class="skeleton mt-2 h-3 w-4/5"></div>
				<div class="skeleton mt-1.5 h-2.5 w-1/2"></div>
			</div>
		</div>

		<div
			v-else
			class="grid grid-cols-3 gap-x-3 gap-y-5 px-4 md:grid-cols-5 md:gap-x-4 md:px-6 lg:grid-cols-6 lg:px-8 xl:grid-cols-7"
		>
			<div v-for="index in props.count" :key="index">
				<div class="skeleton aspect-[2/3] w-full"></div>
				<div class="skeleton mt-2 h-3 w-4/5"></div>
				<div class="skeleton mt-1.5 h-2.5 w-1/2"></div>
			</div>
		</div>
	</section>
</template>
