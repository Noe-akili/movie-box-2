<script setup>
import { computed, onMounted, onUnmounted, ref } from "vue";
import { remoteTransferFileUrl } from "../api/client";
import AppIcon from "../components/AppIcon.vue";
import QrCode from "../components/QrCode.vue";
import QrScanner from "../components/QrScanner.vue";
import { useDownloads } from "../stores/downloads";
import { startTransferPolling, stopTransferPolling, useTransfer } from "../stores/transfer";
import { toast } from "../stores/toast";
import { formatSize } from "../util/format";

/**
 * Ecran « Transferts » : heberge une salle de partage sur le Wi-Fi local.
 * Plusieurs appareils du meme reseau peuvent y deposer et y recuperer des fichiers.
 */
const {
	running,
	url,
	files,
	received,
	shared,
	sharedAll,
	remote,
	remoteFiles,
	lastError,
	isLoading,
	start,
	stop,
	setShared,
	shareAll,
	join,
	leave,
	sendToRemote,
	removeReceived,
	clearReceived,
} = useTransfer();
const { completedItems, refresh } = useDownloads();

const shareable = computed(() => completedItems.value);
const sharedSet = computed(() => new Set(shared.value));

const scannerOpen = ref(false);
const joinUrl = ref("");
const joining = ref(false);
const sendingId = ref(null);

const onStart = async () => {
	try {
		await start();
		toast.info("Partage démarré");
	} catch (error) {
		toast.error(error.message);
	}
};

const onStop = async () => {
	try {
		await stop();
		toast.info("Partage arrêté");
	} catch (error) {
		toast.error(error.message);
	}
};

const toggleShare = async (id) => {
	const next = sharedSet.value.has(id)
		? shared.value.filter((value) => value !== id)
		: [...shared.value, id];
	try {
		await setShared(next);
	} catch (error) {
		toast.error(error.message);
	}
};

const copyLink = async () => {
	const value = url.value;
	if (!value) return;
	try {
		if (navigator.clipboard?.writeText) {
			await navigator.clipboard.writeText(value);
		} else {
			const area = document.createElement("textarea");
			area.value = value;
			document.body.appendChild(area);
			area.select();
			document.execCommand("copy");
			area.remove();
		}
		toast.info("Lien copié");
	} catch {
		toast.error("Copie impossible");
	}
};

const onRemoveReceived = async (id) => {
	try {
		await removeReceived(id);
	} catch (error) {
		toast.error(error.message);
	}
};

const onClearReceived = async () => {
	try {
		await clearReceived();
		toast.info("Fichiers reçus supprimés");
	} catch (error) {
		toast.error(error.message);
	}
};

const onShareAll = async () => {
	try {
		await shareAll();
		toast.info("Toute la bibliothèque est partagée automatiquement");
	} catch (error) {
		toast.error(error.message);
	}
};

const onJoin = async (value = joinUrl.value) => {
	const address = String(value || "").trim();
	if (!address) {
		toast.info("Scanne le QR code ou colle l'adresse de la salle");
		return;
	}

	joining.value = true;
	try {
		await join(address);
		joinUrl.value = "";
		toast.success("Salle rejointe");
	} catch (error) {
		toast.error(error.message);
	} finally {
		joining.value = false;
	}
};

const onDetected = (value) => {
	scannerOpen.value = false;
	joinUrl.value = String(value || "");
	onJoin(joinUrl.value);
};

const onLeave = async () => {
	try {
		await leave();
		toast.info("Salle quittée");
	} catch (error) {
		toast.error(error.message);
	}
};

const onSendToRemote = async (id) => {
	sendingId.value = id;
	try {
		await sendToRemote([id]);
		toast.success("Fichier envoyé à la salle");
	} catch (error) {
		toast.error(error.message);
	} finally {
		sendingId.value = null;
	}
};

const iconFor = (file) => {
	if ((file.mime || "").startsWith("video/")) return "film";
	if ((file.mime || "").startsWith("image/")) return "image";
	return "archive";
};

onMounted(() => {
	refresh();
	startTransferPolling();
});

onUnmounted(() => stopTransferPolling());
</script>

<template>
	<div class="pb-nav">
		<header class="page-top space-y-4 border-b border-white/5 bg-inkSoft/60 px-4 pb-5 md:px-6 lg:px-8">
			<div class="mx-auto flex max-w-[1200px] items-center gap-3">
				<span class="grid h-11 w-11 place-items-center rounded-2xl bg-brand/15 text-brandSoft">
					<AppIcon name="transfer" :size="22" />
				</span>
				<div>
					<h1 class="text-2xl font-bold tracking-tight md:text-3xl">Transferts</h1>
					<p class="mt-1 text-sm text-white/45">
						Partage de fichiers entre les appareils connectés à ce Wi-Fi.
					</p>
				</div>
			</div>
		</header>

		<section class="mx-auto max-w-[1200px] space-y-5 px-4 py-6 md:px-6 lg:px-8">
			<p v-if="lastError" class="rounded-xl border border-amber-500/25 bg-amber-500/5 px-3 py-2 text-xs text-amber-200/80">
				{{ lastError }}
			</p>

			<div class="surface p-5">
				<div class="flex flex-wrap items-center justify-between gap-4">
					<div class="flex items-center gap-3">
						<span
							class="h-2.5 w-2.5 rounded-full"
							:class="running ? 'bg-emerald-400 shadow-glowSoft' : 'bg-white/25'"
						></span>
						<div>
							<p class="font-semibold">{{ running ? "Partage actif" : "Partage arrêté" }}</p>
							<p class="text-xs text-white/45">
								{{ running ? "Les appareils du réseau peuvent échanger des fichiers." : "Démarre pour afficher un lien et un QR code." }}
							</p>
						</div>
					</div>

					<button
						v-if="!running"
						type="button"
						class="btn btn-solid"
						@click="onStart"
					>
						<AppIcon name="transfer" :size="16" />
						<span>Démarrer le partage</span>
					</button>
					<button v-else type="button" class="btn btn-inverse" @click="onStop">
						<AppIcon name="close" :size="16" />
						<span>Arrêter</span>
					</button>
				</div>

				<div v-if="running" class="mt-5 flex flex-col items-center gap-4 sm:flex-row sm:items-start">
					<QrCode :value="url" :size="200" />

					<div class="min-w-0 flex-1 space-y-3">
						<div>
							<p class="mb-1 text-xs uppercase tracking-wide text-white/40">Adresse à ouvrir</p>
							<p class="break-all rounded-xl border border-inkLine bg-inkRaised px-3 py-2 font-mono text-sm text-white/80">
								{{ url }}
							</p>
						</div>
						<div class="flex flex-wrap gap-2">
							<button type="button" class="btn btn-inverse px-3 py-2 text-xs" @click="copyLink">
								<AppIcon name="link" :size="14" />
								<span>Copier le lien</span>
							</button>
							<a class="btn btn-ghost px-3 py-2 text-xs" :href="url" target="_blank" rel="noreferrer">
								<AppIcon name="external" :size="14" />
								<span>Ouvrir la page</span>
							</a>
						</div>
						<p class="text-xs text-white/40">
							Scanne le QR code depuis l'autre appareil, ou saisis l'adresse dans son navigateur.
							Le partage reste sur ton réseau local.
						</p>
					</div>
				</div>
			</div>

			<!-- Rejoindre la salle d'un autre appareil : scan du QR ou adresse. -->
			<div class="surface p-5">
				<div class="mb-3 flex items-center gap-2">
					<span class="grid h-9 w-9 place-items-center rounded-xl bg-brand/15 text-brandSoft">
						<AppIcon name="scan" :size="18" />
					</span>
					<div>
						<h2 class="text-sm font-semibold text-white/80">Rejoindre une salle</h2>
						<p class="text-xs text-white/40">Scanne le QR code affiché sur l'autre appareil.</p>
					</div>
				</div>

				<div v-if="remote" class="rounded-xl border border-emerald-500/25 bg-emerald-500/5 p-3">
					<div class="flex flex-wrap items-center justify-between gap-2">
						<div class="min-w-0">
							<p class="text-xs font-semibold text-emerald-300">Salle rejointe</p>
							<p class="break-all font-mono text-[11px] text-white/50">{{ remote.url }}</p>
						</div>
						<button type="button" class="chip" @click="onLeave">
							<AppIcon name="close" :size="13" />
							Quitter
						</button>
					</div>

					<div class="mt-3 border-t border-white/5 pt-3">
						<p class="mb-2 text-xs font-semibold text-white/70">
							Fichiers de la salle ({{ remoteFiles.length }})
						</p>
						<p v-if="!remoteFiles.length" class="text-xs text-white/40">
							Aucun fichier pour l'instant.
						</p>
						<ul v-else class="max-h-64 space-y-2 overflow-y-auto pr-1">
							<li
								v-for="file in remoteFiles"
								:key="file.id"
								class="flex items-center gap-3 rounded-lg border border-white/5 bg-white/[0.02] px-2.5 py-2"
							>
								<AppIcon :name="iconFor(file)" :size="15" class="text-white/40" />
								<div class="min-w-0 flex-1">
									<p class="truncate text-xs font-semibold text-white/80">{{ file.name }}</p>
									<p class="text-[11px] text-white/40">
										{{ formatSize(file.size) }}
										<span v-if="file.uploader"> · {{ file.uploader }}</span>
									</p>
								</div>
								<a
									class="btn btn-inverse shrink-0 px-3 py-1.5 text-[11px]"
									:href="remoteTransferFileUrl(file.id)"
									:download="file.name"
								>
									<AppIcon name="download" :size="13" />
									<span>Télécharger</span>
								</a>
							</li>
						</ul>
					</div>

					<div v-if="shareable.length" class="mt-3 border-t border-white/5 pt-3">
						<p class="mb-2 text-xs font-semibold text-white/70">Envoyer un de mes téléchargements</p>
						<ul class="max-h-48 space-y-1.5 overflow-y-auto pr-1">
							<li
								v-for="item in shareable"
								:key="`remote-${item.id}`"
								class="flex items-center gap-2 rounded-lg border border-white/5 px-2.5 py-1.5"
							>
								<span class="min-w-0 flex-1 truncate text-[11px] text-white/70">
									{{ item.title || item.fileName }}
								</span>
								<button
									type="button"
									class="chip !py-1 text-[11px]"
									:disabled="sendingId === item.id"
									@click="onSendToRemote(item.id)"
								>
									<AppIcon name="upload" :size="12" />
									{{ sendingId === item.id ? "Envoi…" : "Envoyer" }}
								</button>
							</li>
						</ul>
					</div>
				</div>

				<div v-else class="space-y-3">
					<button type="button" class="btn btn-solid w-full" @click="scannerOpen = true">
						<AppIcon name="camera" :size="16" />
						<span>Scanner un QR code</span>
					</button>
					<div class="flex gap-2">
						<input
							v-model="joinUrl"
							type="text"
							inputmode="url"
							autocomplete="off"
							placeholder="http://192.168.1.20:8090/jeton/"
							class="min-w-0 flex-1 rounded-xl border border-inkLine bg-inkRaised px-3 py-2.5 font-mono text-xs text-white/80 outline-none focus:border-brand/60"
							@keyup.enter="onJoin()"
						/>
						<button
							type="button"
							class="btn btn-inverse shrink-0 px-4 py-2.5 text-xs"
							:disabled="joining"
							@click="onJoin()"
						>
							<AppIcon name="link" :size="14" />
							<span>{{ joining ? "Connexion…" : "Rejoindre" }}</span>
						</button>
					</div>
					<p class="text-xs text-white/40">
						L'adresse est affichée sur l'appareil qui héberge la salle.
					</p>
				</div>
			</div>

			<div class="surface p-5">
				<div class="mb-3 flex flex-wrap items-center justify-between gap-2">
					<h2 class="text-sm font-semibold text-white/80">
						Ma bibliothèque partagée ({{ shared.length }})
					</h2>
					<button
						v-if="shareable.length && !sharedAll"
						type="button"
						class="chip"
						@click="onShareAll"
					>
						<AppIcon name="share" :size="13" />
						Tout partager
					</button>
				</div>
				<p class="mb-3 text-xs text-white/40">
					{{
						sharedAll
							? "Tous tes téléchargements terminés sont exposés automatiquement dans la salle."
							: "Seuls les téléchargements cochés sont exposés dans la salle."
					}}
				</p>
				<p v-if="!shareable.length" class="text-sm text-white/45">
					Aucun téléchargement terminé à partager pour l'instant.
				</p>
				<ul v-else class="max-h-72 space-y-2 overflow-y-auto pr-1">
					<li
						v-for="item in shareable"
						:key="item.id"
						class="flex items-center gap-3 rounded-xl border border-white/5 bg-white/[0.02] px-3 py-2"
					>
						<AppIcon name="archive" :size="16" class="text-white/40" />
						<span class="min-w-0 flex-1 truncate text-sm text-white/80">{{ item.title || item.fileName }}</span>
						<span class="text-xs text-white/40">{{ formatSize(item.bytesTotal) }}</span>
						<button
							type="button"
							class="chip"
							:class="sharedSet.has(item.id) ? 'chip-active' : ''"
							@click="toggleShare(item.id)"
						>
							{{ sharedSet.has(item.id) ? "Partagé" : "Partager" }}
						</button>
					</li>
				</ul>
			</div>

			<div class="surface p-5">
				<div class="mb-3 flex items-center justify-between gap-3">
					<h2 class="text-sm font-semibold text-white/80">
						Fichiers dans la salle ({{ files.length }})
					</h2>
					<span v-if="isLoading" class="text-xs text-white/40">Actualisation…</span>
				</div>
				<p v-if="!files.length" class="text-sm text-white/45">
					{{ running ? "Aucun fichier pour l'instant : les appareils du réseau peuvent en envoyer." : "Démarre le partage pour voir les fichiers de la salle." }}
				</p>
				<ul v-else class="space-y-2">
					<li
						v-for="file in files"
						:key="file.id"
						class="flex items-center gap-3 rounded-xl border border-white/5 bg-white/[0.02] px-3 py-2"
					>
						<AppIcon :name="iconFor(file)" :size="16" class="text-white/40" />
						<div class="min-w-0 flex-1">
							<p class="truncate text-sm text-white/80">{{ file.name }}</p>
							<p class="text-xs text-white/40">
								{{ formatSize(file.size) }}
								<span v-if="file.uploader"> · {{ file.uploader }}</span>
								<span v-if="file.kind === 'download'"> · depuis mes téléchargements</span>
							</p>
						</div>
					</li>
				</ul>
			</div>

			<div class="surface p-5">
				<div class="mb-3 flex items-center justify-between gap-3">
					<h2 class="text-sm font-semibold text-white/80">
						Reçus sur cet appareil ({{ received.length }})
					</h2>
					<button
						v-if="received.length"
						type="button"
						class="btn btn-ghost px-3 py-1.5 text-xs"
						@click="onClearReceived"
					>
						<AppIcon name="trash" :size="14" />
						<span>Tout supprimer</span>
					</button>
				</div>
				<p v-if="!received.length" class="text-sm text-white/45">
					Les fichiers envoyés depuis les autres appareils apparaissent ici.
				</p>
				<ul v-else class="space-y-2">
					<li
						v-for="file in received"
						:key="file.id"
						class="flex items-center gap-3 rounded-xl border border-white/5 bg-white/[0.02] px-3 py-2"
					>
						<AppIcon :name="iconFor(file)" :size="16" class="text-white/40" />
						<div class="min-w-0 flex-1">
							<p class="truncate text-sm text-white/80">{{ file.name }}</p>
							<p class="text-xs text-white/40">
								{{ formatSize(file.size) }}
								<span v-if="file.uploader"> · de {{ file.uploader }}</span>
							</p>
						</div>
						<button
							type="button"
							class="icon-btn h-8 w-8"
							aria-label="Supprimer"
							@click="onRemoveReceived(file.id)"
						>
							<AppIcon name="trash" :size="14" />
						</button>
					</li>
				</ul>
			</div>

			<p class="text-center text-xs text-white/35">
				Le partage s'arrête quand tu appuies sur « Arrêter » ou quand l'application est fermée.
			</p>
		</section>

		<QrScanner v-if="scannerOpen" @detected="onDetected" @close="scannerOpen = false" />
	</div>
</template>
