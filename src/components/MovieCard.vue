<script setup>
import { computed, ref } from "vue";
import imdb from "../images/imdb.svg";
import tomato from "../images/tomato.svg";
import { getGenreName } from "../util/genreList";
import { toast } from "../stores/toast";
import IconHeart from "./icons/IconHeart.vue";
import { useFetchMovie } from "../composables/useFetchMovie";

const props = defineProps({
	movie: { type: Object, required: true },
});

const favorite = ref(false);
const IMAGE_PATH = "https://image.tmdb.org/t/p/w342";

const releaseDate = computed(() => new Date(props.movie.release_date).toUTCString());
const posterUrl = computed(() =>
	props.movie.poster_path
		? `${IMAGE_PATH}${props.movie.poster_path}`
		: "https://via.placeholder.com/400"
);

const favoriteButtonClass = computed(() =>
	favorite.value
		? "bg-brand/20 text-brand hover:bg-brand/30"
		: "bg-white/20 text-lightestGray hover:bg-white/30 hover:text-brand"
);

const toggleFavorite = () => {
	favorite.value = !favorite.value;
	if (favorite.value) toast.success(`Added ${props.movie.title} to favorites`);
	else toast.info(`Removed ${props.movie.title} from favorites`);
};

const { fetch } = useFetchMovie(() => props.movie);
</script>

<template>
	<div
		data-testid="movie-card"
		class="group mx-auto flex w-full max-w-[333px] flex-col gap-2 font-bold sm:max-w-none"
	>
		<div class="relative overflow-hidden rounded-xl bg-slate-200 shadow-card">
			<img
				:src="posterUrl"
				:alt="props.movie.title"
				data-testid="movie-poster"
				class="w-full cursor-pointer select-none transition-transform duration-300 group-hover:scale-[1.03]"
				@click="fetch"
				loading="lazy"
			/>

			<div
				class="pointer-events-none absolute inset-0 bg-gradient-to-t from-black/45 via-black/10 to-transparent opacity-0 transition-opacity duration-300 group-hover:opacity-100"
				aria-hidden="true"
			></div>

			<div
				class="absolute left-3 top-3 rounded-full bg-white/80 px-3 py-1 text-xs font-bold text-darkestGray backdrop-blur-sm shadow-soft"
			>
				TV SERIES
			</div>

			<button
				type="button"
				:aria-pressed="favorite"
				:aria-label="
					favorite
						? `Remove ${props.movie.title} from favorites`
						: `Add ${props.movie.title} to favorites`
				"
				class="absolute right-3 top-3 grid h-9 w-9 place-items-center rounded-full backdrop-blur-md transition-normal focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30"
				:class="favoriteButtonClass"
				@click="toggleFavorite"
			>
				<IconHeart class="mt-[1px]" />
			</button>
		</div>

		<div class="flex items-center justify-between">
			<div data-testid="movie-release-date" class="text-xs text-darkerLightGray">
				{{ releaseDate }}
			</div>
			<div class="text-xs text-darkerLightGray">USA, 2016 - Current</div>
		</div>

		<h3
			data-testid="movie-title"
			class="line-clamp-2 cursor-pointer text-lg text-darkestGray transition-normal group-hover:text-rose"
			@click="fetch"
		>
			{{ props.movie.title }}
		</h3>

		<div class="flex justify-between font-normal">
			<div class="flex items-center space-x-3 text-sm">
				<img :src="imdb" alt="IMDB logo" />
				<span>860/100</span>
			</div>
			<div class="flex items-center space-x-3 text-sm">
				<img :src="tomato" alt="rotten tomatoes" />
				<span>97%</span>
			</div>
		</div>

		<div class="text-xs text-darkerLightGray">
			<span
				v-for="(genre, index) in props.movie.genre_ids"
				:key="genre"
				class="mr-2 text-sm text-darkerGray"
			>
				{{ getGenreName(genre) }}<span v-if="index !== props.movie.genre_ids.length - 1">,</span>
			</span>
		</div>
	</div>
</template>

