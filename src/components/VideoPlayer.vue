<script setup>
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import Artplayer from "artplayer";
import fr from "artplayer/i18n/fr";

// Pas de ±10 s par double-tap : ArtPlayer ne propose que « double-tap = lecture »
// sur mobile et « double-clic = plein écran » sur bureau. On neutralise ces deux
// comportements pour brancher notre propre saut avant/arrière (voir plus bas).
Artplayer.SEEK_STEP = 10;
Artplayer.MOBILE_DBCLICK_PLAY = false;
Artplayer.DBCLICK_FULLSCREEN = false;

/** Saut appliqué par le double-tap/double-clic (±10 s). */
const DBLCLICK_SEEK = 10;

/**
 * Lecteur vidéo basé sur ArtPlayer (remplace la balise <video> native).
 *
 * Apports par rapport au <video> natif : gestes tactiles (double-tap pour
 * avancer/reculer, balayage volume/luminosité), raccourcis clavier, menu de
 * qualité, vitesse de lecture, miroir, Picture-in-Picture, plein écran, rotation
 * automatique et panneau de réglages. Les URL restent celles du backend
 * (Internet Archive en ligne ou fichier local du dossier de téléchargement).
 *
 * Reprise de lecture : la position est mémorisée par vidéo (`id`) dans le
 * stockage local et restaurée automatiquement, si bien qu'appuyer sur Lecture
 * reprend là où l'on s'était arrêté.
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
	/** Identifiant stable de la vidéo (film ou épisode) servant de clé de reprise. */
	id: { type: String, default: "" },
});

const emit = defineEmits(["ready", "error"]);

const container = ref(null);
let player = null;
let saveTimer = null;
let restored = false;

/** Clé de reprise propre à chaque vidéo. */
const progressKey = () => `moviebox:progress:${props.id || props.src}`;

const readProgress = () => {
	try {
		return Number(window.localStorage.getItem(progressKey())) || 0;
	} catch {
		return 0;
	}
};

const writeProgress = (time) => {
	if (!Number.isFinite(time) || time <= 0) return;
	try {
		window.localStorage.setItem(progressKey(), String(Math.floor(time)));
	} catch {
		/* stockage indisponible : on ignore */
	}
};

const clearProgress = () => {
	try {
		window.localStorage.removeItem(progressKey());
	} catch {
		/* stockage indisponible : on ignore */
	}
};

const buildQuality = () => {
	if (!props.qualities || props.qualities.length < 2) return null;
	return props.qualities.map((quality) => ({
		html: quality.html || quality.label || quality.quality || "source",
		url: quality.url,
		default: quality.url === props.src,
	}));
};

/** Replace la tête de lecture là où l'utilisateur s'était arrêté (une seule fois). */
const resume = () => {
	if (restored || !player) return;
	restored = true;
	const saved = readProgress();
	const duration = player.duration || 0;
	// Ignore les toutes premières secondes et la toute fin du générique.
	if (saved > 5 && (!duration || saved < duration - 5)) {
		player.currentTime = saved;
	}
};

/**
 * Double-tap / double-clic : côté gauche = reculer de 10 s, côté droit =
 * avancer de 10 s, au centre = lecture/pause. Le côté est déduit de la position
 * horizontale du clic rapportée à la largeur du lecteur (0 → gauche, 1 → droite).
 */
const handleDoubleClick = (event) => {
	if (!player || !player.duration) return;
	const rect = container.value?.getBoundingClientRect();
	const ratio = rect && rect.width ? (event.clientX - rect.left) / rect.width : 0.5;
	if (ratio < 0.35) player.backward = DBLCLICK_SEEK;
	else if (ratio > 0.65) player.forward = DBLCLICK_SEEK;
	else player.toggle();
};

const create = () => {
	if (!container.value || !props.src) return;

	restored = false;
	const quality = buildQuality();
	player = new Artplayer({
		container: container.value,
		url: props.src,
		poster: props.poster || "",
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
		moreVideoAttr: { playsInline: true },
		...(quality ? { quality } : {}),
	});

	// `loadedmetadata` arrive avant `ready` (qui exige assez de données) : on
	// restaure la position au plus tôt pour que la lecture reprenne au bon endroit.
	player.on("video:loadedmetadata", resume);
	player.on("ready", () => {
		resume();
		emit("ready");
	});
	player.on("video:error", () => emit("error"));
	player.on("dblclick", handleDoubleClick);
	player.on("pause", () => {
		if (player) writeProgress(player.currentTime);
	});
	player.on("video:ended", clearProgress);
	// Sauvegarde régulière (5 s) pendant la lecture.
	player.on("video:timeupdate", () => {
		if (!player || !player.playing || saveTimer) return;
		saveTimer = window.setTimeout(() => {
			saveTimer = null;
			if (player) writeProgress(player.currentTime);
		}, 5000);
	});
};

const destroy = () => {
	if (saveTimer) {
		window.clearTimeout(saveTimer);
		saveTimer = null;
	}
	if (player) {
		// Mémorise la position avant destruction (changement d'épisode, retour…).
		if (player.currentTime) writeProgress(player.currentTime);
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
	<div ref="container" class="h-full w-full bg-black"></div>
</template>
