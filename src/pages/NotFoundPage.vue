<script setup>
import EmptyState from "../components/EmptyState.vue";
</script>

<template>
	<div class="grid min-h-screen place-items-center px-4">
		<EmptyState
			icon="explore"
			title="Page introuvable (404)"
			description="Ce lien n'existe pas ou a été déplacé. Retourne à l'accueil pour continuer à explorer."
			action-label="Retour à l'accueil"
			@action="$router.push('/')"
		/>
	</div>
</template>
