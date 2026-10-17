<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import Artplayer from "artplayer";
import fr from "artplayer/i18n/fr";

/**
 * Lecteur vidéo basé sur ArtPlayer (remplace la balise <video> native).
 *
 * Apports par rapport au <video> natif : gestes tactiles (double-tap pour
 * avancer/reculer, balayage volume/luminosité), raccourcis clavier, menu de
 * qualité, vitesse de lecture, miroir, Picture-in-Picture, plein écran, rotation
 * automatique et panneau de réglages. Les URL restent celles du backend
 * (Internet Archive en ligne ou fichier local du dossier de téléchargement).
 */
const props = defineProps({
	/** URL lisible par la balise vidéo (déjà absolue si nécessaire). */
	src: { type: String, required: true },
	/** Image d'attente affichée avant la lecture. */
	poster: { type: String, default: "" },
	/** Démarre la lecture automatiquement (peut être bloqué par le navigateur). */
	autoplay: { type: Boolean, default: false },
	/**
	 * Qualités proposées dans le menu de réglages :
	 * `[{ html: "720p · mp4", url: "…" }, …]` (2 entrées minimum pour l'afficher).
	 */
	qualities: { type: Array, default: () => [] },
});

const emit = defineEmits(["ready", "error"]);

const container = ref(null);
let player = null;

const buildQuality = () => {
	if (!props.qualities || props.qualities.length < 2) return undefined;
	return props.qualities.map((quality) => ({
		html: quality.html || quality.label || quality.quality || "source",
		url: quality.url,
		default: quality.url === props.src,
	}));
};

const create = () => {
	if (!container.value || !props.src) return;
	player = new Artplayer({
		container: container.value,
		url: props.src,
		poster: props.poster || undefined,
		autoplay: props.autoplay,
		muted: false,
		volume: 0.8,
		theme: "#22C55E",
		lang: "fr",
		i18n: { fr },
		hotkey: true,
		pip: true,
		fullscreen: true,
		fullscreenWeb: true,
		setting: true,
		playbackRate: true,
		aspectRatio: true,
		autoOrientation: true,
		airplay: true,
		miniProgressBar: true,
		playsInline: true,
		gesture: true,
		autoSize: false,
		quality: buildQuality(),
		moreVideoAttr: { playsInline: true },
	});

	player.on("ready", () => emit("ready"));
	player.on("video:error", () => emit("error"));
};

const destroy = () => {
	if (player) {
		player.destroy();
		player = null;
	}
};

onMounted(create);
onBeforeUnmount(destroy);

// Une nouvelle source (changement d'épisode ou de qualité) recharge le lecteur.
watch(
	() => props.src,
	() => {
		destroy();
		create();
	},
);
</script>

<template>
	<div ref="container" class="h-full w-full"></div>
</template>
