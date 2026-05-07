<script setup>
import { RouterView } from "vue-router";
import Loader from "./components/Loader.vue";
import ToastHost from "./components/ToastHost.vue";
import { useLoading } from "./stores/loading";

const isLoading = useLoading();
</script>

<template>
	<ToastHost />
	<Loader v-if="isLoading" />
	<RouterView />
</template>

