/**
 * Défilement infini : charge la suite quand le bas de la page devient visible.
 *
 * `onLoadMore` est appelé une seule fois par « seuil » atteint (on attend la fin
 * du chargement précédent), ce qui évite les appels en rafale pendant le scroll.
 * Le témoin est renvoyé par la fonction : à poser sur un élément du template
 * (`ref="sentinel"`).
 */
import { onBeforeUnmount, onMounted, ref } from "vue";

export const useInfiniteScroll = (onLoadMore, { enabled, rootMargin = "600px" } = {}) => {
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

	onMounted(() => {
		if (typeof IntersectionObserver === "undefined") return;

		observer = new IntersectionObserver(
			(entries) => {
				if (entries[0]?.isIntersecting) run();
			},
			{ rootMargin: `${rootMargin} 0px` },
		);

		if (sentinel.value) observer.observe(sentinel.value);
	});

	onBeforeUnmount(() => observer?.disconnect());

	return { sentinel, isLoading };
};
