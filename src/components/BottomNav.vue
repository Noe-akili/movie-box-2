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
		class="fixed inset-x-0 bottom-0 z-40 border-t border-white/5 bg-ink/90 pb-safe backdrop-blur-xl lg:hidden"
		aria-label="Navigation principale"
	>
		<ul class="mx-auto flex max-w-xl items-stretch justify-between px-1.5 pt-1.5">
			<li v-for="item in items" :key="item.to" class="flex-1">
				<RouterLink
					:to="item.to"
					class="group relative flex flex-col items-center gap-1 rounded-2xl px-1 py-1.5 text-[10px] font-semibold transition-normal"
					:class="isActive(item.to) ? 'text-brandSoft' : 'text-white/45 hover:text-white/80'"
				>
					<span
						class="relative grid h-9 w-14 place-items-center rounded-xl transition-all duration-300 ease-out"
						:class="isActive(item.to) ? 'bg-brand/15 shadow-glowSoft' : 'group-hover:bg-white/5'"
					>
						<AppIcon
							:name="item.name"
							:size="21"
							:filled="isActive(item.to)"
							class="transition-transform duration-200 ease-out"
							:class="isActive(item.to) ? 'scale-110' : 'scale-100'"
						/>
						<span
							v-if="item.to === '/downloads' && activeCount"
							class="absolute -right-0.5 -top-0.5 grid h-4 min-w-[1rem] place-items-center rounded-full bg-brand px-1 text-[9px] font-bold leading-none text-white ring-2 ring-ink"
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
