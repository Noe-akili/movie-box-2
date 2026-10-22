/**
 * Défilement infini : charge la suite quand le bas de la page devient visible.
 *
 * `onLoadMore` est appelé une seule fois par « seuil » atteint (on attend la fin
 * du chargement précédent), ce qui évite les appels en rafale pendant le scroll.
 * Le témoin est renvoyé par la fonction : à poser sur un élément du template
 * (`ref="sentinel"`). Comme le témoin est souvent rendu après le premier
 * chargement (squelette puis résultats), on l'observe dès qu'il apparaît.
 */
import { onBeforeUnmount, ref, watch } from "vue";
import { infiniteRootMargin } from "../util/perf";

export const useInfiniteScroll = (onLoadMore, { enabled, rootMargin } = {}) => {
	// Marge réduite sur appareil faible : on charge moins de contenu hors écran.
	const margin = rootMargin ?? infiniteRootMargin();
	const sentinel = ref(null);
	const isLoading = ref(false);
	let observer = null;

	const run = async () => {
		if (isLoading.value) return;
		if (typeof enabled === "function" && !enabled.value) return;

		isLoading.value = true;
		try {
			await onLoadMore();
		} finally {
			isLoading.value = false;
		}
	};

	const ensureObserver = () => {
		if (observer || typeof IntersectionObserver === "undefined") return;
		observer = new IntersectionObserver(
			(entries) => {
				if (entries.some((entry) => entry.isIntersecting)) run();
			},
			{ rootMargin: `${margin} 0px` },
		);
	};

	// Le témoin peut être monté/démonté au fil des états (chargement, résultats
	// vides, changement de filtre) : on (ré)observe à chaque apparition.
	watch(
		sentinel,
		(element, previous) => {
			if (previous) observer?.unobserve(previous);
			if (!element) return;
			ensureObserver();
			observer?.observe(element);
		},
		{ flush: "post" },
	);

	onBeforeUnmount(() => observer?.disconnect());

	return { sentinel, isLoading };
};
