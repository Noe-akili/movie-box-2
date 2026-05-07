import axios from "axios";
import { useRouter } from "vue-router";
import { useLoadingUpdate } from "../stores/loading";
import { toast } from "../stores/toast";
import { setCurrentMovie } from "../stores/movie";

const API_URL = "https://api.themoviedb.org/3";

export const useFetchMovie = (getMovie) => {
	const router = useRouter();
	const setLoading = useLoadingUpdate();

	const fetchMovie = async () => {
		const movie = typeof getMovie === "function" ? getMovie() : getMovie;
		if (!movie?.id) return;

		try {
			const { data } = await axios.get(`${API_URL}/movie/${movie.id}`, {
				params: {
					api_key: import.meta.env.VITE_API_KEY,
				},
			});

			if (data) {
				setCurrentMovie(data);
				router.push({ name: "movie", params: { id: data.imdb_id } });
				window.setTimeout(() => setLoading(false), 2000);
			}
		} catch (error) {
			window.setTimeout(() => {
				setLoading(false);
				toast.error(`Error: ${String(error)}`);
			}, 2000);
		}
	};

	const fetch = () => {
		setLoading(true);
		fetchMovie();
	};

	return { fetch };
};

