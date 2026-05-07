<script setup>
import axios from "axios";
import { computed, onMounted, ref } from "vue";
import Footer from "../components/Footer.vue";
import Logo from "../components/Logo.vue";
import MovieDetails from "../components/MovieDetails.vue";
import MovieList from "../components/MovieList.vue";
import SearchBar from "../components/SearchBar.vue";
import SignIn from "../components/SignIn.vue";
import errorImage from "../images/error.jpg";
import { useLoadingUpdate } from "../stores/loading";
import { toast } from "../stores/toast";

const API_URL = "https://api.themoviedb.org/3";
const IMAGE_PATH = "https://image.tmdb.org/t/p/w1280";

const movies = ref([]);
const selectedMovie = ref(null);
const searchKey = ref("");
const moviesListHeading = ref("Top 10 Movies");

const setLoading = useLoadingUpdate();

const fetchMovies = async (key = "") => {
	const type = key ? "search/movie" : "movie/popular";

	try {
		const {
			data: { results },
		} = await axios.get(`${API_URL}/${type}`, {
			params: {
				api_key: import.meta.env.VITE_API_KEY,
				query: key,
			},
		});

		if (results) window.setTimeout(() => setLoading(false), 1000);

		const featured = results.slice(0, 10);
		const randomMovie = Math.floor(Math.random() * results.length);

		if (key) {
			moviesListHeading.value = `Search Results (${results.length})`;
			movies.value = results;
			selectedMovie.value = results[0] || null;
		} else {
			movies.value = featured;
			selectedMovie.value = results[randomMovie] || null;
		}
	} catch (error) {
		window.setTimeout(() => {
			setLoading(false);
			toast.error(`Error: ${String(error)}`);
		}, 1000);
	}
};

const searchMovies = async () => {
	setLoading(true);
	await fetchMovies(searchKey.value);
	searchKey.value = "";
	window.setTimeout(() => setLoading(false), 1000);
};

onMounted(() => {
	fetchMovies();
});

const heroStyle = computed(() => {
	const backdrop = selectedMovie.value?.backdrop_path;
	return {
		backgroundImage: backdrop
			? `url(${IMAGE_PATH}${backdrop})`
			: `url(${errorImage})`,
	};
});
</script>

<template>
	<main class="font-dmSans">
		<section
			class="relative flex min-h-screen flex-col bg-slate-400 bg-cover bg-center bg-no-repeat"
			:style="heroStyle"
		>
			<div class="hero-overlay" aria-hidden="true"></div>
			<div class="hero-bottom-fade" aria-hidden="true"></div>

			<header class="relative z-10 pt-4 md:pt-5 lg:pt-6">
				<div
					class="container grid grid-cols-2 grid-rows-2 items-center gap-4 md:gap-5 lg:flex lg:gap-6"
				>
					<Logo />
					<SignIn />
					<SearchBar v-model="searchKey" @submit="searchMovies" />
				</div>
			</header>

			<MovieDetails :selectedMovie="selectedMovie" />
		</section>

		<MovieList :movies="movies" :heading="moviesListHeading" />
		<Footer />
	</main>
</template>

