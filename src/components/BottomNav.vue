<script setup>
import { useRoute } from "vue-router";
import AppIcon from "./AppIcon.vue";
import { useDownloads } from "../stores/downloads";

const route = useRoute();
const { activeCount } = useDownloads();

const items = [
	{ name: "home", label: "Accueil", to: "/" },
	{ name: "film", label: "Films", to: "/movies" },
	{ name: "tv", label: "Séries", to: "/series" },
	{ name: "gift", label: "Gratuit", to: "/free" },
	{ name: "bookmark", label: "Ma liste", to: "/library" },
	{ name: "download", label: "Téléch.", to: "/downloads" },
];

const isActive = (to) => (to === "/" ? route.path === "/" : route.path.startsWith(to));
</script>

<template>
	<nav
		class="fixed inset-x-0 bottom-0 z-40 border-t border-white/5 bg-ink/95 pb-safe backdrop-blur-xl lg:hidden"
		aria-label="Navigation principale"
	>
		<ul class="mx-auto flex max-w-xl items-stretch justify-between px-1">
			<li v-for="item in items" :key="item.to" class="flex-1">
				<RouterLink
					:to="item.to"
					class="flex flex-col items-center gap-1 rounded-xl px-1 py-2.5 text-[10px] font-medium transition-normal"
					:class="isActive(item.to) ? 'text-brandSoft' : 'text-white/45 hover:text-white/80'"
				>
					<span class="relative">
						<AppIcon :name="item.name" :size="21" :filled="isActive(item.to)" />
						<span
							v-if="item.to === '/downloads' && activeCount"
							class="absolute -right-2 -top-1.5 grid h-4 min-w-[1rem] place-items-center rounded-full bg-brand px-1 text-[9px] font-bold leading-none text-white"
						>
							{{ activeCount }}
						</span>
					</span>
					<span>{{ item.label }}</span>
				</RouterLink>
			</li>
		</ul>
	</nav>
</template>
