import { createRouter, createWebHistory } from "vue-router";
import HomePage from "../pages/HomePage.vue";
import MoviePage from "../pages/MoviePage.vue";

const router = createRouter({
	history: createWebHistory(),
	routes: [
		{ path: "/", name: "home", component: HomePage },
		{ path: "/movie/:id", name: "movie", component: MoviePage },
	],
	scrollBehavior() {
		return { top: 0 };
	},
});

export default router;

