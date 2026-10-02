<script setup>
import { onBeforeUnmount, onMounted, ref } from "vue";
import { RouterLink, useRoute } from "vue-router";
import AppIcon from "./AppIcon.vue";
import Logo from "./Logo.vue";
import { useDownloads } from "../stores/downloads";

const props = defineProps({
	overlay: { type: Boolean, default: false },
});

const route = useRoute();
const { activeCount } = useDownloads();
const scrolled = ref(false);

const onScroll = () => {
	scrolled.value = window.scrollY > 40;
};

onMounted(() => {
	window.addEventListener("scroll", onScroll, { passive: true });
	onScroll();
});

onBeforeUnmount(() => window.removeEventListener("scroll", onScroll));

const links = [
	{ label: "Accueil", to: "/" },
	{ label: "Films", to: "/movies" },
	{ label: "Séries", to: "/series" },
	{ label: "Gratuit", to: "/free" },
	{ label: "Téléchargements", to: "/downloads" },
];

const isActive = (to) => (to === "/" ? route.path === "/" : route.path.startsWith(to));
</script>

<template>
	<header
		class="fixed inset-x-0 top-0 z-40 transition-normal"
		:class="
			props.overlay && !scrolled
				? 'bg-gradient-to-b from-ink/90 via-ink/40 to-transparent'
				: 'border-b border-white/5 bg-ink/90 backdrop-blur-xl'
		"
	>
		<div class="mx-auto flex h-14 max-w-[1800px] items-center gap-3 px-4 md:h-16 md:px-6 lg:px-8">
			<Logo />

			<nav class="ml-6 hidden items-center gap-1 lg:flex" aria-label="Navigation bureau">
				<RouterLink
					v-for="link in links"
					:key="link.to"
					:to="link.to"
					class="rounded-lg px-3 py-2 text-sm font-medium transition-normal"
					:class="isActive(link.to) ? 'bg-white/10 text-white' : 'text-white/60 hover:bg-white/5 hover:text-white'"
				>
					{{ link.label }}
				</RouterLink>
			</nav>

			<div class="ml-auto flex items-center gap-2">
				<RouterLink to="/search" class="icon-btn" aria-label="Rechercher un film, une série">
					<AppIcon name="search" :size="18" />
				</RouterLink>
				<RouterLink to="/downloads" class="icon-btn relative" aria-label="Téléchargements">
					<AppIcon name="download" :size="18" />
					<span
						v-if="activeCount"
						class="absolute -right-1 -top-1 grid h-4 min-w-[1rem] place-items-center rounded-full bg-brand px-1 text-[9px] font-bold leading-none text-white"
					>
						{{ activeCount }}
					</span>
				</RouterLink>
				<RouterLink to="/library" class="icon-btn hidden sm:grid" aria-label="Ma liste">
					<AppIcon name="bookmark" :size="18" />
				</RouterLink>
				<RouterLink
					to="/profile"
					class="grid h-10 w-10 place-items-center rounded-full bg-brand-gradient text-sm font-bold text-white shadow-glow"
					aria-label="Mon profil"
				>
					<AppIcon name="user" :size="18" />
				</RouterLink>
			</div>
		</div>
	</header>
</template>
