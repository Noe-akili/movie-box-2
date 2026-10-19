<script setup>
import { computed } from "vue";
import { createQr } from "../util/qr";

/**
 * QR code rendu en SVG (généré localement, sans réseau ni dépendance).
 * On laisse une marge blanche de 4 modules (zone de silence) et un fond clair,
 * indispensable pour que l'appareil photo du voisin puisse faire la mise au point.
 */
const props = defineProps({
	value: { type: String, default: "" },
	size: { type: [Number, String], default: 220 },
});

const quiet = 4;

const qr = computed(() => (props.value ? createQr(props.value) : null));

const viewBox = computed(() => {
	const dimension = (qr.value?.size || 21) + quiet * 2;
	return `0 0 ${dimension} ${dimension}`;
});

/** Chemin unique regroupant tous les modules sombres (rendu plus rapide). */
const path = computed(() => {
	if (!qr.value) return "";
	let d = "";
	const { modules, size } = qr.value;
	for (let row = 0; row < size; row += 1) {
		for (let col = 0; col < size; col += 1) {
			if (modules[row][col]) d += `M${col + quiet} ${row + quiet}h1v1h-1z`;
		}
	}
	return d;
});
</script>

<template>
	<div
		class="inline-flex items-center justify-center rounded-2xl bg-white p-3"
		:style="{ width: `${size}px`, height: `${size}px` }"
	>
		<svg v-if="qr" :viewBox="viewBox" class="h-full w-full" role="img" aria-label="QR code">
			<rect :width="qr.size + quiet * 2" :height="qr.size + quiet * 2" fill="#ffffff" />
			<path :d="path" fill="#0b0d12" shape-rendering="crispEdges" />
		</svg>
		<span v-else class="px-2 text-center text-xs text-ink">Lien trop long pour le QR code</span>
	</div>
</template>
