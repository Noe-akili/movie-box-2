<script setup>
import { computed } from "vue";

/**
 * Jeu d'icônes SVG de l'interface (style outline 24x24).
 * Une seule source de vérité évite de multiplier les fichiers pour chaque pictogramme.
 */
const PATHS = {
	home: ["M3 10.75 12 3.5l9 7.25", "M5.5 9.75V20.5h13V9.75"],
	film: ["M4 4h16v16H4z", "M9 4v16M15 4v16", "M4 9h5M15 9h5M4 15h5M15 15h5"],
	tv: ["M4 7h16v12H4z", "M8 3l4 4 4-4"],
	gift: ["M20 12v8.5H4V12", "M2.5 8h19v4h-19z", "M12 8v12", "M12 8S10.6 3 8.2 3a2.4 2.4 0 0 0 0 5H12Zm0 0s1.4-5 3.8-5a2.4 2.4 0 0 1 0 5H12Z"],
	bookmark: ["M6 3.5h12v17l-6-4.6-6 4.6z"],
	user: ["M20 21v-2a4 4 0 0 0-4-4H8a4 4 0 0 0-4 4v2", "M12 11.5a4 4 0 1 0 0-8 4 4 0 0 0 0 8Z"],
	play: ["M8 5.2v13.6L19 12z"],
	pause: ["M9 5h2v14H9zM13 5h2v14h-2z"],
	plus: ["M12 5v14M5 12h14"],
	check: ["M20 6.5 9.2 17.3 4 12.1"],
	close: ["M18 6 6 18M6 6l12 12"],
	chevronLeft: ["M15 18.5 8.5 12 15 5.5"],
	chevronRight: ["M9 5.5 15.5 12 9 18.5"],
	chevronDown: ["M6 9.5l6 6 6-6"],
	chevronUp: ["M18 14.5l-6-6-6 6"],
	search: ["M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16Z", "M21 21l-4.3-4.3"],
	star: ["M12 3.6l2.7 5.5 6 .9-4.3 4.3 1 6-5.4-2.9-5.4 2.9 1-6L3.3 10l6-.9z"],
	info: ["M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z", "M12 16.5V11M12 7.8h.01"],
	external: ["M14 4h6v6", "M20 4l-8.5 8.5", "M18 14v5.5H4.5V6H10"],
	clock: ["M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z", "M12 7.5V12l3.2 2"],
	calendar: ["M4 5.5h16v15H4z", "M8 3v4M16 3v4M4 10.5h16"],
	download: ["M12 3.5v11", "M7.5 10.5 12 15l4.5-4.5", "M4.5 19.5h15"],
	list: ["M8 6h13M8 12h13M8 18h13", "M3.5 6h.01M3.5 12h.01M3.5 18h.01"],
	heart: ["M12 20.3s-7.2-4.6-7.2-9.7A4.2 4.2 0 0 1 12 7.8a4.2 4.2 0 0 1 7.2 2.8c0 5.1-7.2 9.7-7.2 9.7Z"],
	flame: ["M12 2.8s5 5.4 5 9.4a5 5 0 0 1-10 0c0-1.3.4-2.5 1.1-3.6", "M12 21.2a3 3 0 0 0 3-3c0-1.7-3-4.2-3-4.2s-3 2.5-3 4.2a3 3 0 0 0 3 3Z"],
	globe: ["M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Z", "M3.5 9h17M3.5 15h17", "M12 3c2.5 2.6 3.8 5.6 3.8 9S14.5 18.4 12 21c-2.5-2.6-3.8-5.6-3.8-9S9.5 5.6 12 3Z"],
	filter: ["M4 5.5h16l-6.2 7.2v5.6l-3.6-2v-3.6z"],
	explore: ["M4 4.5h7v7H4zM13 4.5h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z"],
	archive: ["M3.5 7.5h17v13h-17z", "M3 3.5h18V7H3z", "M10 11.5h4"],
	image: ["M4 5h16v14H4z", "M4 16.5l4.5-4.5 4 4 3-3L20 16.5", "M9.5 9.5h.01"],
	link: ["M10.5 13.5a4 4 0 0 0 5.7 0l2.6-2.6a4 4 0 1 0-5.7-5.7L11.8 6.5", "M13.5 10.5a4 4 0 0 0-5.7 0l-2.6 2.6a4 4 0 1 0 5.7 5.7l1.3-1.3"],
	trash: ["M4 7h16", "M9.5 7V4.5h5V7", "M6.5 7l1 13h9l1-13"],
	arrowLeft: ["M20 12H4", "M10.5 5.5 4 12l6.5 6.5"],
	arrowRight: ["M4 12h16", "M13.5 5.5 20 12l-6.5 6.5"],
	eye: ["M2.5 12S6.2 6.4 12 6.4 21.5 12 21.5 12 17.8 17.6 12 17.6 2.5 12 2.5 12Z", "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z"],
	sparkles: ["M12 3.5l1.8 4.3 4.3 1.7-4.3 1.8L12 15.5l-1.8-4.2L5.9 9.5l4.3-1.7z", "M18.2 15.2l.9 2.1 2.1.9-2.1.9-.9 2.1-.9-2.1-2.1-.9 2.1-.9z"],
};

const props = defineProps({
	name: { type: String, required: true },
	size: { type: [Number, String], default: 20 },
	strokeWidth: { type: [Number, String], default: 1.7 },
	filled: { type: Boolean, default: false },
});

const paths = computed(() => {
	const value = PATHS[props.name];
	if (!value) return [];
	return Array.isArray(value) ? value : [value];
});
</script>

<template>
	<svg
		xmlns="http://www.w3.org/2000/svg"
		:width="props.size"
		:height="props.size"
		viewBox="0 0 24 24"
		:fill="props.filled ? 'currentColor' : 'none'"
		stroke="currentColor"
		:stroke-width="props.strokeWidth"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
		focusable="false"
		class="shrink-0"
	>
		<path v-for="(path, index) in paths" :key="index" :d="path" />
	</svg>
</template>
