<script setup>
import logo from "../images/logo.png";

const props = defineProps({
	color: { type: String, default: "" },
	// Sur mobile, seule l'icone est affichee (comme la maquette de reference).
	hideText: { type: Boolean, default: false },
});
</script>

<template>
	<RouterLink to="/" class="flex shrink-0 items-center space-x-2 sm:space-x-3">
		<img :src="logo" alt="logo" class="h-8 w-8 rounded-xl object-cover ring-1 ring-white/10 md:h-10 md:w-10" />
		<span
			class="text-lg font-bold sm:text-xl md:text-2xl"
			:class="[props.color || 'text-white', props.hideText ? 'hidden sm:inline' : '']"
		>
			MovieBox
		</span>
	</RouterLink>
</template>

