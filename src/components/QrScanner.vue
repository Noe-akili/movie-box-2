<script setup>
import jsQR from "jsqr";
import { onBeforeUnmount, onMounted, ref } from "vue";
import AppIcon from "./AppIcon.vue";

/**
 * Scanner de QR code via la caméra arrière.
 *
 * `getUserMedia` passe par la WebView Tauri : Wry relaie la demande à Android
 * (permission CAMERA déclarée dans le manifeste par le script de patch CI).
 * On échantillonne l'image ~10 fois par seconde pour rester économe en batterie.
 */
const emit = defineEmits(["detected", "close"]);

const video = ref(null);
const canvas = ref(null);
const error = ref("");
const starting = ref(true);

let stream = null;
let frame = null;
let stopped = false;
let lastScan = 0;

const stop = () => {
	stopped = true;
	if (frame !== null) cancelAnimationFrame(frame);
	frame = null;
	if (stream) {
		for (const track of stream.getTracks()) track.stop();
		stream = null;
	}
};

const close = () => {
	stop();
	emit("close");
};

const scan = (timestamp) => {
	if (stopped) return;

	if (timestamp - lastScan > 100) {
		lastScan = timestamp;
		const source = video.value;
		const target = canvas.value;

		if (source && target && source.readyState >= 2 && source.videoWidth > 0) {
			const width = source.videoWidth;
			const height = source.videoHeight;
			const scale = Math.min(1, 720 / Math.max(width, height));
			target.width = Math.max(1, Math.round(width * scale));
			target.height = Math.max(1, Math.round(height * scale));

			const context = target.getContext("2d", { willReadFrequently: true });
			context.drawImage(source, 0, 0, target.width, target.height);
			const image = context.getImageData(0, 0, target.width, target.height);
			const code = jsQR(image.data, image.width, image.height, {
				inversionAttempts: "dontInvert",
			});

			if (code?.data) {
				stop();
				emit("detected", code.data);
				return;
			}
		}
	}

	frame = requestAnimationFrame(scan);
};

const start = async () => {
	try {
		if (!navigator.mediaDevices?.getUserMedia) {
			throw new Error("La caméra n'est pas disponible sur cet appareil.");
		}

		stream = await navigator.mediaDevices.getUserMedia({
			video: { facingMode: { ideal: "environment" } },
			audio: false,
		});

		if (stopped) {
			for (const track of stream.getTracks()) track.stop();
			return;
		}

		video.value.srcObject = stream;
		await video.value.play();
		starting.value = false;
		frame = requestAnimationFrame(scan);
	} catch (caught) {
		starting.value = false;
		error.value =
			caught?.name === "NotAllowedError"
				? "Accès caméra refusé. Autorise la caméra dans les réglages Android."
				: caught?.name === "NotFoundError"
					? "Aucune caméra détectée sur cet appareil."
					: caught?.message || "Impossible d'ouvrir la caméra.";
	}
};

onMounted(start);
onBeforeUnmount(stop);
</script>

<template>
	<Teleport to="body">
		<div class="fixed inset-0 z-[70] flex flex-col bg-black">
			<header class="flex items-center justify-between gap-3 px-4 pb-3 pt-[calc(1rem+var(--safe-top))]">
				<div class="flex items-center gap-2">
					<AppIcon name="scan" :size="18" class="text-brandSoft" />
					<p class="text-sm font-semibold text-white">Scanner le QR code de la salle</p>
				</div>
				<button type="button" class="icon-btn h-9 w-9" aria-label="Fermer" @click="close">
					<AppIcon name="close" :size="18" />
				</button>
			</header>

			<div class="relative min-h-0 flex-1 overflow-hidden">
				<video
					ref="video"
					class="h-full w-full object-cover"
					autoplay
					muted
					playsinline
				></video>
				<canvas ref="canvas" class="hidden"></canvas>

				<!-- Viseur -->
				<div class="pointer-events-none absolute inset-0 grid place-items-center">
					<div class="relative h-[62vw] max-h-[320px] w-[62vw] max-w-[320px]">
						<span class="absolute left-0 top-0 h-8 w-8 rounded-tl-xl border-l-4 border-t-4 border-brandSoft"></span>
						<span class="absolute right-0 top-0 h-8 w-8 rounded-tr-xl border-r-4 border-t-4 border-brandSoft"></span>
						<span class="absolute bottom-0 left-0 h-8 w-8 rounded-bl-xl border-b-4 border-l-4 border-brandSoft"></span>
						<span class="absolute bottom-0 right-0 h-8 w-8 rounded-br-xl border-b-4 border-r-4 border-brandSoft"></span>
					</div>
				</div>

				<p
					v-if="starting && !error"
					class="absolute inset-x-0 bottom-24 text-center text-sm text-white/70"
				>
					Ouverture de la caméra…
				</p>

				<div
					v-if="error"
					class="absolute inset-x-5 top-1/2 -translate-y-1/2 rounded-2xl border border-amber-500/30 bg-inkCard/95 px-4 py-5 text-center"
				>
					<AppIcon name="camera" :size="26" class="mx-auto mb-2 text-amber-300" />
					<p class="text-sm text-white/85">{{ error }}</p>
					<p class="mt-2 text-xs text-white/45">
						Tu peux aussi saisir l'adresse de la salle à la main.
					</p>
				</div>
			</div>

			<p class="px-6 pb-[calc(1.25rem+var(--safe-bottom))] pt-4 text-center text-xs text-white/50">
				Place le QR code affiché sur l'autre appareil dans le cadre.
			</p>
		</div>
	</Teleport>
</template>
