<script setup>
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from "vue";
import PosterCard from "./PosterCard.vue";
import { mediaKey } from "../util/media";
import { gridOverscan } from "../util/perf";

const props = defineProps({
	items: { type: Array, default: () => [] },
});

/*
	Grille virtualisée : seules les rangées proches de l'écran sont montées.
	Sur un téléphone ancien, cela évite de garder des centaines de posters
	(et leurs images décodées) en mémoire pendant le scroll infini.
*/
const container = ref(null);
const grid = ref(null);

const containerWidth = ref(0);
const visibleStartRow = ref(0);
const visibleEndRow = ref(1);
const rowHeight = ref(180);
const gapY = ref(20);

const overscan = computed(() => gridOverscan());

const columnsForWidth = (width) => {
	if (width >= 1440) return 7;
	if (width >= 1200) return 6;
	if (width >= 768) return 5;
	if (width >= 512) return 4;
	return 3;
};

const columnCount = computed(() => columnsForWidth(containerWidth.value));
const totalRows = computed(() =>
	Math.max(0, Math.ceil(props.items.length / Math.max(1, columnCount.value))),
);
const rowStride = computed(() => Math.max(1, rowHeight.value + gapY.value));

const visibleStart = computed(() =>
	totalRows.value ? Math.min(visibleStartRow.value, totalRows.value - 1) : 0,
);
const visibleEnd = computed(() => {
	if (!totalRows.value) return 0;
	const end = Math.max(visibleStart.value + 1, Math.min(visibleEndRow.value, totalRows.value));
	return Math.min(end, totalRows.value);
});

const startIndex = computed(() => visibleStart.value * columnCount.value);
const endIndex = computed(() =>
	Math.min(props.items.length, visibleEnd.value * columnCount.value),
);
const visibleItems = computed(() => props.items.slice(startIndex.value, endIndex.value));

const topSpacer = computed(() => visibleStart.value * rowStride.value);
const bottomSpacer = computed(() =>
	Math.max(0, (totalRows.value - visibleEnd.value) * rowStride.value),
);

const sizes = computed(() => {
	const cols = columnCount.value;
	// Largeur approximative d'une carte selon les breakpoints de la grille.
	const viewportShare = { 3: 31, 4: 23, 5: 18, 6: 15, 7: 13 }[cols] || 18;
	return `(max-width: 511px) 31vw, (max-width: 767px) 23vw, (max-width: 1199px) 18vw, (max-width: 1439px) 15vw, ${viewportShare}vw`;
});

const keyFor = (item) => {
	const key = mediaKey(item) ?? item?.id ?? item?.title;
	return `${item?.source || item?.mediaType || "x"}:${key}`;
};

let raf = 0;

const update = () => {
	raf = 0;
	const element = container.value;
	if (!element) return;

	const rows = totalRows.value;
	if (!rows) {
		if (visibleStartRow.value !== 0 || visibleEndRow.value !== 0) {
			visibleStartRow.value = 0;
			visibleEndRow.value = 0;
		}
		return;
	}

	const rect = element.getBoundingClientRect();
	const stride = rowStride.value;
	const viewTop = Math.max(0, -rect.top);
	const viewBottom = Math.min(
		rect.height || 0,
		Math.max(0, (typeof window !== "undefined" ? window.innerHeight : 0) - rect.top),
	);

	// Garde toujours une rangée montée : sa hauteur sert de référence et évite
	// un conteneur vide quand le scroll dépasse temporairement la liste.
	let start = Math.floor(viewTop / stride) - overscan.value;
	let end = Math.ceil(viewBottom / stride) + overscan.value;
	start = Math.max(0, Math.min(start, rows - 1));
	end = Math.max(start + 1, Math.min(end, rows));

	if (start !== visibleStartRow.value) visibleStartRow.value = start;
	if (end !== visibleEndRow.value) visibleEndRow.value = end;
};

const scheduleUpdate = () => {
	if (raf) return;
	raf = window.requestAnimationFrame(update);
};

const measure = async () => {
	await nextTick();
	const gridElement = grid.value;
	if (!gridElement) return;

	const style = window.getComputedStyle(gridElement);
	const gap = parseFloat(style.rowGap);
	if (Number.isFinite(gap)) gapY.value = gap;

	const first = gridElement.firstElementChild;
	if (first) {
		const height = first.getBoundingClientRect().height;
		if (height > 4) rowHeight.value = height;
	}

	update();
};

const measureWidth = () => {
	const element = container.value;
	if (!element) return;
	const width = element.clientWidth;
	if (width && width !== containerWidth.value) {
		containerWidth.value = width;
		nextTick(measure);
	}
};

let resizeObserver = null;

onMounted(() => {
	measureWidth();
	if (!containerWidth.value && typeof window !== "undefined") {
		containerWidth.value = window.innerWidth;
	}

	window.addEventListener("scroll", scheduleUpdate, { passive: true });
	window.addEventListener("resize", measureWidth, { passive: true });

	if (typeof ResizeObserver !== "undefined" && container.value) {
		resizeObserver = new ResizeObserver(measureWidth);
		resizeObserver.observe(container.value);
	}

	nextTick(measure);
	document.fonts?.ready.then(measure).catch(() => {});
});

onBeforeUnmount(() => {
	window.removeEventListener("scroll", scheduleUpdate);
	window.removeEventListener("resize", measureWidth);
	resizeObserver?.disconnect();
	if (raf) window.cancelAnimationFrame(raf);
});

watch(
	() => props.items,
	() => nextTick(measure),
	{ flush: "post" },
);

watch(columnCount, () => nextTick(measure));

watch(
	() => props.items.length,
	() => scheduleUpdate(),
);
</script>

<template>
	<div ref="container" class="relative">
		<div aria-hidden="true" :style="{ height: `${topSpacer}px` }"></div>

		<div
			ref="grid"
			class="grid auto-rows-fr grid-cols-3 gap-x-3 gap-y-5 sm:grid-cols-4 md:grid-cols-5 md:gap-x-4 lg:grid-cols-6 xl:grid-cols-7"
		>
			<PosterCard
				v-for="(item, index) in visibleItems"
				:key="keyFor(item)"
				:item="item"
				width="w-full"
				:sizes="sizes"
				:priority="startIndex + index < 6"
			/>
		</div>

		<div aria-hidden="true" :style="{ height: `${bottomSpacer}px` }"></div>
	</div>
</template>
