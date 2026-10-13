<script setup>
import { computed, ref, watch } from "vue";
import { RouterView, useRoute } from "vue-router";
import BottomNav from "./components/BottomNav.vue";
import Loader from "./components/Loader.vue";
import ToastHost from "./components/ToastHost.vue";
import TopBar from "./components/TopBar.vue";
import { ensureDownloadsPolling } from "./stores/downloads";
import { useLoading } from "./stores/loading";

const route = useRoute();
const isLoading = useLoading();

// Suit la progression des telechargements en arriere-plan (badge + onglet dedie).
ensureDownloadsPolling();

// La page de lecture s'affiche en plein ecran (sans barre de navigation).
const showChrome = computed(() => route.meta?.chrome !== false);
const overlayTopBar = computed(() => route.meta?.overlay === true);

// Ordre des onglets : sert a donner un sens de glissement entre les transitions.
const TAB_ORDER = ["home", "movies", "series", "free", "library", "downloads", "profile"];
const tabIndex = (name) => TAB_ORDER.indexOf(name);
const transitionName = ref("fade-up");

watch(
	() => route.name,
	(to, from) => {
		const toTab = tabIndex(to);
		const fromTab = tabIndex(from);
		if (toTab !== -1 && fromTab !== -1 && toTab !== fromTab) {
			// Onglet vers onglet : glissement horizontal selon le sens de navigation.
			transitionName.value = toTab > fromTab ? "tab-left" : "tab-right";
		} else {
			// Vers une fiche / la lecture : leger fondu vertical.
			transitionName.value = "fade-up";
		}
	},
);
</script>

<template>
	<ToastHost />
	<Loader v-if="isLoading" />

	<TopBar v-if="showChrome" :overlay="overlayTopBar" />

	<main class="min-h-screen bg-ink">
		<RouterView v-slot="{ Component }">
			<Transition :name="transitionName" mode="out-in">
				<component :is="Component" :key="route.path" />
			</Transition>
		</RouterView>
	</main>

	<BottomNav v-if="showChrome" />
</template>
