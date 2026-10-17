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
		class="fixed inset-x-0 top-0 z-40 transition-all duration-300"
		:class="
			props.overlay && !scrolled
				? 'bg-gradient-to-b from-inkDeep/95 via-ink/45 to-transparent'
				: 'border-b border-white/5 bg-ink/80 shadow-topbar backdrop-blur-xl'
		"
	>
		<div class="mx-auto flex h-16 max-w-[1800px] items-center gap-2 px-4 md:h-[72px] md:gap-4 md:px-6 lg:px-8">
			<Logo />

			<nav class="ml-4 hidden items-center gap-1 self-stretch lg:flex" aria-label="Navigation bureau">
				<RouterLink
					v-for="link in links"
					:key="link.to"
					:to="link.to"
					class="nav-link grid place-items-center"
					:class="isActive(link.to) ? 'nav-link-active' : ''"
				>
					{{ link.label }}
					<span
						class="absolute inset-x-3 bottom-2 h-0.5 rounded-full bg-brand-gradient transition-all duration-300"
						:class="isActive(link.to) ? 'opacity-100' : 'opacity-0'"
					></span>
				</RouterLink>
			</nav>

			<div class="ml-auto flex items-center gap-2">
				<RouterLink to="/search" class="icon-btn" aria-label="Rechercher un film, une série">
					<AppIcon name="search" :size="19" />
				</RouterLink>
				<RouterLink to="/library" class="icon-btn" aria-label="Ma liste">
					<AppIcon name="bookmark" :size="19" />
				</RouterLink>
				<RouterLink to="/downloads" class="icon-btn relative" aria-label="Téléchargements">
					<AppIcon name="download" :size="19" />
					<span
						v-if="activeCount"
						class="absolute -right-1 -top-1 grid h-4 min-w-[1rem] place-items-center rounded-full bg-brand px-1 text-[9px] font-bold leading-none text-white ring-2 ring-ink"
					>
						{{ activeCount }}
					</span>
				</RouterLink>
				<RouterLink
					to="/profile"
					class="ml-1 grid h-10 w-10 place-items-center rounded-full bg-brand-gradient text-white shadow-glowSoft ring-1 ring-white/20 transition-normal hover:scale-105"
					aria-label="Mon profil"
				>
					<AppIcon name="user" :size="19" />
				</RouterLink>
			</div>
		</div>
	</header>
</template>
