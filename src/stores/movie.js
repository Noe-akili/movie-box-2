import { ref } from "vue";

const currentMovie = ref(null);

export const useCurrentMovie = () => currentMovie;

export const setCurrentMovie = (movie) => {
	currentMovie.value = movie || null;
};

