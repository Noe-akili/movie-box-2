<script setup>
import logo from "../images/tv.svg";
import { computed } from "vue";

const props = defineProps({
	color: { type: String, default: "" },
});

const textColor = computed(() => props.color || "text-white");
</script>

<template>
	<RouterLink
		to="/"
		class="flex items-center space-x-2 sm:space-x-3 md:space-x-5"
	>
		<div class="w-9 md:w-auto">
			<img :src="logo" alt="logo" />
		</div>
		<span class="text-lg font-bold md:text-2xl sm:text-xl" :class="textColor">
			MovieBox
		</span>
	</RouterLink>
</template>

