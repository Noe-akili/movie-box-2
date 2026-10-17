<script setup>
import { onBeforeUnmount, onMounted, nextTick, ref, watch } from "vue";
import { useRoute } from "vue-router";
import AppIcon from "./AppIcon.vue";

/**
 * Menu contextuel « … » (trois points) réutilisable.
 *
 * Le panneau est rendu dans `<body>` (Teleport) en position `fixed` : il n'est
 * donc jamais rogné par un parent `overflow-hidden` / `overflow-y-auto`, et il
 * reste au-dessus de tout (`z-[100]`, plus haut que la navigation et les modales).
 *
 * Le placement est calculé selon l'écran : à droite si la place existe, sinon à
 * gauche ; sous le bouton, ou au-dessus s'il n'y a pas la place en bas. L'écran
 * est ainsi toujours entièrement visible, quel que soit l'endroit du bouton.
 */
const props = defineProps({
	label: { type: String, default: "Actions" },
	icon: { type: String, default: "more" },
	// Côté préféré du panneau : « end » aligne son bord droit au bouton.
	align: { type: String, default: "end" },
	width: { type: Number, default: 224 },
});

const route = useRoute();

const isOpen = ref(false);
const isReady = ref(false);
const triggerRef = ref(null);
const menuRef = ref(null);
const position = ref({ top: "-9999px", left: "-9999px" });

const MARGIN = 8;

/** Place le panneau en fonction de la position du bouton et de l'écran. */
const computePosition = () => {
	const trigger = triggerRef.value;
	const menu = menuRef.value;
	if (!trigger || !menu) return;

	const rect = trigger.getBoundingClientRect();
	const menuWidth = menu.offsetWidth || props.width;
	const menuHeight = menu.offsetHeight || 0;
	const viewportWidth = window.innerWidth;
	const viewportHeight = window.innerHeight;

	// Axe horizontal : préférence (end = bord droit aligné), puis repli/flip.
	let left = props.align === "start" ? rect.left : rect.right - menuWidth;
	if (left + menuWidth + MARGIN > viewportWidth) left = viewportWidth - menuWidth - MARGIN;
	if (left < MARGIN) left = MARGIN;

	// Axe vertical : sous le bouton, ou au-dessus s'il n'y a pas la place.
	let top = rect.bottom + MARGIN;
	const overflowsBottom = top + menuHeight + MARGIN > viewportHeight;
	const fitsAbove = rect.top - menuHeight - MARGIN >= MARGIN;
	if (overflowsBottom && fitsAbove) {
		top = rect.top - menuHeight - MARGIN;
	} else if (menuHeight && overflowsBottom) {
		// Aucun côté parfait : on borne pour rester dans l'écran.
		top = Math.max(MARGIN, viewportHeight - menuHeight - MARGIN);
	}

	position.value = { top: `${Math.round(top)}px`, left: `${Math.round(left)}px` };
	isReady.value = true;
};

const openMenu = async () => {
	isOpen.value = true;
	isReady.value = false;
	await nextTick();
	computePosition();
};

const closeMenu = () => {
	isOpen.value = false;
	isReady.value = false;
};

const toggle = () => (isOpen.value ? closeMenu() : openMenu());

/** Ferme le menu au clic extérieur (ou sur un élément non marqué `data-keep-open`). */
const onDocumentPointerDown = (event) => {
	if (!isOpen.value) return;
	const target = event.target;
	if (triggerRef.value?.contains(target)) return;
	if (menuRef.value?.contains(target)) return;
	closeMenu();
};

const onMenuClick = (event) => {
	if (event.target.closest("[data-keep-open]")) return;
	closeMenu();
};

const onKeydown = (event) => {
	if (event.key === "Escape") closeMenu();
};

const onReflow = () => {
	if (isOpen.value) computePosition();
};

onMounted(() => {
	document.addEventListener("pointerdown", onDocumentPointerDown, true);
	window.addEventListener("keydown", onKeydown);
	window.addEventListener("resize", onReflow);
	window.addEventListener("scroll", onReflow, true);
});

onBeforeUnmount(() => {
	document.removeEventListener("pointerdown", onDocumentPointerDown, true);
	window.removeEventListener("keydown", onKeydown);
	window.removeEventListener("resize", onReflow);
	window.removeEventListener("scroll", onReflow, true);
});

// Un changement de page referme le menu.
watch(() => route.fullPath, closeMenu);
</script>

<template>
	<div class="relative shrink-0">
		<button
			ref="triggerRef"
			type="button"
			class="icon-btn h-9 w-9"
			:aria-label="props.label"
			:aria-expanded="isOpen"
			aria-haspopup="menu"
			@click.stop="toggle"
		>
			<slot name="trigger">
				<AppIcon :name="props.icon" :size="18" />
			</slot>
		</button>

		<Teleport to="body">
			<div
				v-if="isOpen"
				ref="menuRef"
				class="fixed z-[100] max-h-[70vh] overflow-y-auto rounded-2xl border border-inkLine bg-inkCard p-1.5 shadow-deck"
				:class="isReady ? '' : 'invisible'"
				:style="{ ...position, width: `${props.width}px` }"
				role="menu"
				@click="onMenuClick"
			>
				<slot :close="closeMenu" />
			</div>
		</Teleport>
	</div>
</template>
