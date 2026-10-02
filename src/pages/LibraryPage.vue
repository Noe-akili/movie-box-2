<script setup>
import { computed, ref } from "vue";
import AppIcon from "../components/AppIcon.vue";
import EmptyState from "../components/EmptyState.vue";
import MediaGrid from "../components/MediaGrid.vue";
import { useLibrary } from "../stores/library";

const { favorites, history, clearHistory, clearFavorites } = useLibrary();

const TABS = [
	{ value: "favorites", label: "Ma liste" },
	{ value: "history", label: "Historique" },
];

const activeTab = ref("favorites");
const items = computed(() => (activeTab.value === "favorites" ? favorites.value : history.value));
</script>

<template>
	<div class="pb-nav">
		<header class="space-y-4 border-b border-white/5 bg-inkSoft/60 px-4 pb-5 pt-20 md:px-6 md:pt-24 lg:px-8">
			<div class="mx-auto flex max-w-[1800px] flex-wrap items-end justify-between gap-4">
				<div>
					<h1 class="text-2xl font-bold tracking-tight md:text-3xl">Ma bibliothèque</h1>
					<p class="mt-1 text-sm text-white/45">
						{{ favorites.length }} titre(s) enregistré(s) · {{ history.length }} vu(s) récemment
					</p>
				</div>

				<button
					v-if="items.length"
					type="button"
					class="btn btn-inverse px-3 py-2 text-xs"
					@click="activeTab === 'favorites' ? clearFavorites() : clearHistory()"
				>
					<AppIcon name="trash" :size="14" />
					<span>{{ activeTab === "favorites" ? "Vider Ma liste" : "Vider l'historique" }}</span>
				</button>
			</div>

			<div class="mx-auto max-w-[1800px]">
				<div class="rail -mx-1 px-1">
					<button
						v-for="tab in TABS"
						:key="tab.value"
						type="button"
						class="chip"
						:class="activeTab === tab.value ? 'chip-active' : ''"
						@click="activeTab = tab.value"
					>
						{{ tab.label }}
					</button>
				</div>
			</div>
		</header>

		<section class="mx-auto max-w-[1800px] px-4 py-6 md:px-6 lg:px-8">
			<MediaGrid v-if="items.length" :items="items" />

			<EmptyState
				v-else-if="activeTab === 'favorites'"
				icon="bookmark"
				title="Ma liste est vide"
				description="Touche le cœur sur un film ou une série pour le retrouver ici, même hors ligne."
				action-label="Explorer les films"
				@action="$router.push('/movies')"
			/>

			<EmptyState
				v-else
				icon="clock"
				title="Aucun historique"
				description="Les titres que tu consultes ou regardes apparaîtront ici pour reprendre la lecture en un tap."
				action-label="Aller à l'accueil"
				@action="$router.push('/')"
			/>
		</section>
	</div>
</template>
