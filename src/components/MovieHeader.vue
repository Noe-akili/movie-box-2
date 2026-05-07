<script setup>
import star from "../images/star.svg";

const props = defineProps({
	title: { type: String, default: "" },
	release_date: { type: String, default: "" },
	runtime: { type: [String, Number], default: "" },
	genres: { type: Array, default: () => [] },
	vote_average: { type: [String, Number], default: "" },
	vote_count: { type: [String, Number], default: "" },
});
</script>

<template>
	<div
		class="flex flex-col space-y-3 text-[#404040] font-medium lg:flex-row lg:items-center lg:gap-2 lg:space-y-0"
	>
		<div class="flex flex-wrap gap-2">
			<h3 data-testid="movie-title">{{ props.title }}</h3>
			<span>•</span>
			<p data-testid="movie-release-date">{{ props.release_date }}</p>
			<span>•</span>
			<div>
				<span data-testid="movie-release-runtime">{{ props.runtime }}</span>
				<span> minutes</span>
			</div>
		</div>
		<div
			class="flex flex-wrap items-center justify-between gap-y-3 lg:flex-1"
		>
			<div class="flex space-x-2">
				<div
					v-for="genre in props.genres"
					:key="genre.id"
					class="rounded-2xl border border-borderGray px-4 py-1 text-sm text-rose"
				>
					{{ genre.name }}
				</div>
			</div>
			<div class="flex space-x-2 lg:ml-auto">
				<div class="w-5">
					<img :src="star" alt="star" />
				</div>
				<div class="flex space-x-1 text-darkGray">
					<span class="text-lightestGray">{{ props.vote_average }}</span>
					<span>|</span>
					<span>{{ props.vote_count }}</span>
				</div>
			</div>
		</div>
	</div>
</template>

