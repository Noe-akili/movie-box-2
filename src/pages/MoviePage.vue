<script setup>
import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import formatNumber from "../util/formatNumber";
import MovieHeader from "../components/MovieHeader.vue";
import Logo from "../components/Logo.vue";
import SideBar from "../components/SideBar.vue";
import moreMovies from "../images/more-movies.png";
import menu from "../images/menu.svg";
import IconArrowDown from "../components/icons/IconArrowDown.vue";
import IconList from "../components/icons/IconList.vue";
import IconTicket from "../components/icons/IconTicket.vue";
import { useCurrentMovie } from "../stores/movie";

const router = useRouter();
const movieRef = useCurrentMovie();
const isMenuOpen = ref(false);

const movie = computed(() => movieRef.value);

const releaseDate = computed(() =>
	movie.value ? new Date(movie.value.release_date).toUTCString() : ""
);
const voteAverage = computed(() =>
	movie.value ? Number(movie.value.vote_average).toFixed(1) : ""
);
const voteCount = computed(() => {
	if (!movie.value) return 0;
	return movie.value.vote_count < 1000
		? movie.value.vote_count
		: formatNumber(movie.value.vote_count);
});

const IMAGE_PATH = "https://image.tmdb.org/t/p/w1280";
const backdropStyle = computed(() =>
	movie.value?.backdrop_path
		? { backgroundImage: `url(${IMAGE_PATH}${movie.value.backdrop_path})` }
		: {}
);
</script>

<template>
	<main
		class="font-poppins flex h-screen flex-col overflow-y-hidden space-y-6 md:flex-row md:space-y-0"
	>
		<SideBar :isMenuOpen="isMenuOpen" />

		<header class="pt-4 md:hidden">
			<div class="container flex items-center justify-between">
				<Logo color="text-black" />
				<button type="button" @click="isMenuOpen = !isMenuOpen">
					<img :src="menu" alt="menu" />
				</button>
			</div>
		</header>

		<section class="flex-1 h-full overflow-y-scroll pt-4 md:pt-5 lg:pt-6">
			<div v-if="!movie" class="container py-10">
				<div class="rounded-2xl border border-black/10 bg-white p-6 shadow-soft">
					<p class="text-darkGray">No movie selected.</p>
					<button
						type="button"
						class="btn btn-solid mt-4"
						@click="router.push({ name: 'home' })"
					>
						Back to Home
					</button>
				</div>
			</div>

			<div v-else class="container space-y-8 pb-16">
				<div
					class="relative aspect-video overflow-hidden rounded-2xl bg-slate-400 bg-cover bg-center bg-no-repeat shadow-card lg:aspect-[1198_/_449]"
					:style="backdropStyle"
				>
					<div
						class="absolute inset-0 bg-gradient-to-t from-black/40 via-black/10 to-transparent"
						aria-hidden="true"
					></div>
				</div>

				<MovieHeader
					:title="movie.title"
					:release_date="releaseDate"
					:runtime="movie.runtime"
					:genres="movie.genres"
					:vote_average="voteAverage"
					:vote_count="voteCount"
				/>

				<div class="space-y-16 text-[#666] lg:flex lg:flex-row lg:gap-4 lg:space-y-0">
					<div class="lg:flex lg:w-[60%] lg:flex-col lg:justify-between">
						<p data-testid="movie-overview">{{ movie.overview }}</p>
						<ul class="mt-5 space-y-2 lg:space-y-6">
							<li>
								<span>Director: </span>
								<span class="text-rose">Joseph Kosinski</span>
							</li>
							<li>
								<span>Writers: </span>
								<span class="text-rose">Jim Cash, Jack Epps Jr, Peter Craig</span>
							</li>
							<li>
								<span>Stars: </span>
								<span class="text-rose">Tom Cruise, Jennifer Connelly, Miles Teller</span>
							</li>
						</ul>

						<div
							class="mx-auto mt-7 flex max-w-lg flex-col space-y-4 lg:mx-0 lg:max-w-none lg:flex-row lg:space-y-0 lg:rounded-lg lg:border lg:border-lightGray"
						>
							<button type="button" class="btn bg-rose text-white justify-center">
								Top rated movie #{{ movie.id }}
							</button>
							<button
								type="button"
								class="flex flex-1 items-center justify-between rounded-lg border border-lightGray px-4 py-2 transition-normal hover:text-black lg:rounded-none lg:border-none"
							>
								<span>Awards &amp; Nominations</span>
								<IconArrowDown />
							</button>
						</div>
					</div>

					<div class="space-y-6 lg:w-[40%]">
						<div class="mx-auto max-w-lg space-y-4 lg:mx-0 lg:max-w-none">
							<button type="button" class="btn btn-solid w-full justify-center">
								<IconTicket />
								<span>See Showtimes</span>
							</button>
							<button type="button" class="btn btn-inverse w-full justify-center">
								<IconList />
								<span>More watch options</span>
							</button>
						</div>

						<div class="relative mx-auto max-w-max lg:mx-0 lg:max-w-none">
							<img :src="moreMovies" alt="more movies" class="lg:w-full" />
							<div
								class="absolute bottom-0 left-0 right-0 flex items-center justify-center space-x-2 rounded-b-lg bg-[#12121280] py-2 text-xs font-medium text-[#E8E8E8]"
							>
								<IconList />
								<span>The Best Movies and Shows in September</span>
							</div>
						</div>
					</div>
				</div>
			</div>
		</section>
	</main>
</template>

