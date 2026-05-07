<script setup>
import MovieCard from "./MovieCard.vue";
import IconArrowRight from "./icons/IconArrowRight.vue";

const props = defineProps({
	movies: { type: Array, default: () => [] },
	heading: { type: String, default: "" },
});
</script>

<template>
	<section class="my-12 md:mt-14 lg:mt-16 lg:mb-24">
		<div class="container">
			<div class="mb-8 flex items-center justify-between gap-4 md:mb-12">
				<h2 class="text-xl font-bold sm:text-2xl md:text-3xl lg:text-4xl">
					{{ props.heading }}
				</h2>
				<button
					type="button"
					class="group flex items-center gap-2 text-rose transition-normal hover:text-rose/80"
				>
					<span class="font-normal sm:text-lg">See more</span>
					<IconArrowRight
						class="transition-transform duration-200 group-hover:translate-x-1"
					/>
				</button>
			</div>

			<ul
				v-if="props.movies.length > 0"
				class="grid grid-cols-2 gap-x-4 gap-y-10 sm:gap-x-8 sm:gap-y-12 md:grid-cols-3 lg:grid-cols-4 lg:gap-x-10 lg:gap-y-16"
			>
				<li v-for="movie in props.movies" :key="movie.id">
					<MovieCard :movie="movie" />
				</li>
			</ul>
			<p v-else class="text-darkerGray">Movie not found.</p>
		</div>
	</section>
</template>

