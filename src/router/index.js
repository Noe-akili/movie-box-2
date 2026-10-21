import { createRouter, createWebHistory } from "vue-router";
import HomePage from "../pages/HomePage.vue";
import NotFoundPage from "../pages/NotFoundPage.vue";

const BrowsePage = () => import("../pages/BrowsePage.vue");
const FreePage = () => import("../pages/FreePage.vue");
const SearchPage = () => import("../pages/SearchPage.vue");
const DetailPage = () => import("../pages/DetailPage.vue");
const WatchPage = () => import("../pages/WatchPage.vue");
const LibraryPage = () => import("../pages/LibraryPage.vue");
const DownloadsPage = () => import("../pages/DownloadsPage.vue");
const TransfersPage = () => import("../pages/TransfersPage.vue");
const ProfilePage = () => import("../pages/ProfilePage.vue");

const routes = [
	{
		path: "/",
		name: "home",
		component: HomePage,
		meta: { title: "Accueil", overlay: true },
	},
	{
		path: "/movies",
		name: "movies",
		component: BrowsePage,
		props: { mediaType: "movie" },
		meta: { title: "Films" },
	},
	{
		path: "/series",
		name: "series",
		component: BrowsePage,
		props: { mediaType: "tv" },
		meta: { title: "Séries" },
	},
	{
		path: "/free",
		name: "free",
		component: FreePage,
		meta: { title: "Films gratuits" },
	},
	{
		path: "/search",
		name: "search",
		component: SearchPage,
		meta: { title: "Recherche" },
	},
	{
		path: "/library",
		name: "library",
		component: LibraryPage,
		meta: { title: "Ma bibliothèque" },
	},
	{
		path: "/downloads",
		name: "downloads",
		component: DownloadsPage,
		meta: { title: "Téléchargements" },
	},
	{
		path: "/transfers",
		name: "transfers",
		component: TransfersPage,
		meta: { title: "Transferts" },
	},
	{
		path: "/profile",
		name: "profile",
		component: ProfilePage,
		meta: { title: "Mon profil" },
	},
	{
		path: "/title/:mediaType(movie|tv|archive|internal)/:id",
		name: "detail",
		component: DetailPage,
		props: true,
		meta: { title: "Détails" },
	},
	{
		path: "/watch/:mediaType(movie|tv|archive|internal)/:id",
		name: "watch",
		component: WatchPage,
		props: true,
		meta: { title: "Lecture", chrome: false },
	},
	// Fichier deja telecharge par le backend : lecture hors ligne.
	{
		path: "/watch/local/:id",
		name: "watch-local",
		component: WatchPage,
		props: (route) => ({ mediaType: "local", id: String(route.params.id) }),
		meta: { title: "Lecture hors ligne", chrome: false },
	},
	// Fichier reçu via la salle Wi-Fi locale : lecture depuis le stockage de l'app.
	{
		path: "/watch/received/:id",
		name: "watch-received",
		component: WatchPage,
		props: (route) => ({ mediaType: "received", id: String(route.params.id) }),
		meta: { title: "Fichier reçu", chrome: false },
	},
	// Anciens liens /movie/:id conserves pour ne pas casser les favoris existants.
	{
		path: "/movie/:id",
		redirect: (to) => ({ name: "detail", params: { mediaType: "movie", id: to.params.id } }),
	},
	{
		path: "/:pathMatch(.*)*",
		name: "not-found",
		component: NotFoundPage,
		meta: { title: "Page introuvable" },
	},
];

const router = createRouter({
	history: createWebHistory(),
	routes,
	scrollBehavior(to, from, savedPosition) {
		if (savedPosition) return savedPosition;
		if (to.name === from.name && to.name === "search") return false;
		return { top: 0 };
	},
});

router.afterEach((to) => {
	const base = "moviebox-2";
	document.title = to.meta?.title ? `${to.meta.title} · ${base}` : base;
});

export default router;
