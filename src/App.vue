<script setup>
import { computed } from "vue";
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
</script>

<template>
	<ToastHost />
	<Loader v-if="isLoading" />

	<TopBar v-if="showChrome" :overlay="overlayTopBar" />

	<main class="min-h-screen bg-ink">
		<RouterView />
	</main>

	<BottomNav v-if="showChrome" />
</template>
