<script setup>
import imdb from "../images/imdb.svg";
import tomato from "../images/tomato.svg";
import IconPlayCircle from "./icons/IconPlayCircle.vue";
import { useFetchMovie } from "../composables/useFetchMovie";

const props = defineProps({
	selectedMovie: { type: Object, default: null },
});

const { fetch } = useFetchMovie(() => props.selectedMovie);
</script>

<template>
	<section
		v-if="props.selectedMovie"
		class="relative z-10 flex flex-1 items-center pt-6 text-white md:pt-10"
	>
		<div class="container">
			<div class="max-w-[440px] space-y-3 lg:max-w-[520px] lg:space-y-4">
				<h1
					class="font-bold text-4xl sm:text-5xl lg:text-6xl lg:leading-[1.2]"
				>
					{{ props.selectedMovie.title }}
				</h1>
				<div class="flex items-center space-x-9">
					<div class="flex items-center space-x-3 text-sm font-normal">
						<img :src="imdb" alt="IMDB logo" />
						<span>860/100</span>
					</div>
					<div class="flex items-center space-x-3 text-sm font-normal">
						<img :src="tomato" alt="rotten tomatoes" />
						<span>97%</span>
					</div>
				</div>
				<p
					class="max-w-[420px] font-medium text-white/90 lg:max-w-[520px] lg:text-lg"
				>
					{{ props.selectedMovie.overview }}
				</p>
				<button
					type="button"
					class="btn bg-rose/95 shadow-soft ring-1 ring-white/10 transition-normal hover:bg-rose focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30"
					@click="fetch"
				>
					<IconPlayCircle />
					<span class="text-sm font-bold tracking-wide">WATCH TRAILER</span>
				</button>
			</div>
		</div>
	</section>
</template>

