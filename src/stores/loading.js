import { computed, ref } from "vue";

const isLoading = ref(false);

// Requetes en vol : pilote la barre de progression fine en haut de l'ecran.
// Chaque appel Axios incremente puis decremente le compteur, ce qui donne un
// retour visuel immediat a chaque requete (l'utilisateur ne croit pas que
// l'app a plante pendant un appel lent).
const pendingCount = ref(0);
const hasPending = computed(() => pendingCount.value > 0);

export const useLoading = () => isLoading;

export const useLoadingUpdate = () => (value) => {
	isLoading.value = Boolean(value);
};

export const usePending = () => ({
	pendingCount,
	hasPending,
	beginRequest: () => {
		pendingCount.value += 1;
	},
	endRequest: () => {
		pendingCount.value = Math.max(0, pendingCount.value - 1);
	},
});
