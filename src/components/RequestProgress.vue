<script setup>
import { computed } from "vue";
import { usePending } from "../stores/loading";

/**
 * Barre de progression fine (style Netflix) : chaque requête API lance une
 * animation en haut de l'écran. Elle se complète dès qu'une réponse arrive et
 * disparaît quand la dernière requête est terminée — l'utilisateur voit que
 * l'application travaille, même sur un appel lent.
 */
const { pendingCount, hasPending } = usePending();

// Plus la requête dure, plus la barre avance vite (plafonné avant 90 %).
const width = computed(() => {
	if (!hasPending.value) return 0;
	// Une requête : lente ; plusieurs en parallèle : plus rapide.
	const speed = Math.min(0.7, 0.25 + pendingCount.value * 0.15);
	return Math.min(90, Math.round((0.3 + speed) * 100));
});
</script>

<template>
	<Transition
		enter-active-class="transition-opacity duration-200"
		leave-active-class="transition-opacity duration-300"
		enter-from-class="opacity-0"
		leave-to-class="opacity-0"
	>
		<div v-if="hasPending" class="pointer-events-none fixed inset-x-0 top-0 z-50 h-0.5 bg-white/10">
			<div class="h-full bg-brand-gradient transition-all duration-500 ease-out" :style="{ width: `${width}%` }"></div>
		</div>
	</Transition>
</template>
