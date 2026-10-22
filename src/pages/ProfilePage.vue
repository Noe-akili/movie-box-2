<script setup>
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import {
	apiBase,
	fetchBackendDiag,
	fetchHealth,
	isTauriRuntime,
	serverOrigin,
} from "../api/client";
import AppIcon from "../components/AppIcon.vue";
import DocumentationModal from "../components/DocumentationModal.vue";
import { useLibrary } from "../stores/library";
import { LANGUAGES, REGIONS, updateSettings, useSettings } from "../stores/settings";
import { toast } from "../stores/toast";

const settings = useSettings();
const { favorites, history, clearFavorites, clearHistory } = useLibrary();

const health = ref(null);
const healthError = ref(null);
const showDocs = ref(false);

const diag = ref(null);
const diagError = ref(null);
const diagLoading = ref(false);
let diagTimer = null;

const stats = computed(() => [
	{ label: "Favoris", value: favorites.value.length },
	{ label: "Historique", value: history.value.length },
]);

const runtimeInfo = computed(() => [
	{ label: "Runtime Tauri", value: isTauriRuntime() ? "oui" : "non (navigateur)" },
	{
		label: "Origine média",
		value:
			(typeof window !== "undefined" && window.__SERVER_ORIGIN__) || serverOrigin() || "—",
	},
	{ label: "Base API", value: apiBase() || "—" },
	{ label: "Version Tauri", value: diag.value?.tauriVersion || "—" },
	{ label: "Plateforme", value: diag.value?.platform || "—" },
	{
		label: "Routeur backend",
		value: diag.value ? (diag.value.routerReady ? "prêt" : "absent") : "—",
	},
]);

const onLanguage = (event) => {
	updateSettings({ language: event.target.value });
	toast.success("Langue du catalogue mise à jour");
};

const onRegion = (event) => {
	updateSettings({ region: event.target.value });
	toast.success("Région des offres légales mise à jour");
};

const onPerformance = (event) => {
	updateSettings({ performanceMode: event.target.value });
	toast.success("Mode de performance mis à jour");
};

const checkServer = async () => {
	healthError.value = null;

	try {
		health.value = await fetchHealth();
	} catch (error) {
		health.value = null;
		healthError.value = error.message;
	}
};

const setDeveloper = (value) => updateSettings({ developer: value });

const refreshDiag = async () => {
	diagLoading.value = true;
	diagError.value = null;
	try {
		diag.value = await fetchBackendDiag();
	} catch (error) {
		diagError.value = error?.message || String(error);
	} finally {
		diagLoading.value = false;
	}
};

const copyDiag = async () => {
	try {
		await navigator.clipboard.writeText(diag.value?.log || "");
		toast.success("Journal copié");
	} catch {
		toast.error("Copie impossible");
	}
};

// Rafraîchissement automatique du journal tant que le mode développeur est actif.
watch(
	() => settings.value.developer,
	(enabled) => {
		if (diagTimer) {
			clearInterval(diagTimer);
			diagTimer = null;
		}
		if (enabled) {
			refreshDiag();
			diagTimer = setInterval(refreshDiag, 3000);
		}
	},
	{ immediate: true },
);

onMounted(checkServer);
onUnmounted(() => {
	if (diagTimer) clearInterval(diagTimer);
});
</script>


<template>
	<div class="pb-nav">
		<header class="page-top border-b border-white/5 bg-inkSoft/60 px-4 pb-6 md:px-6 lg:px-8">
			<div class="mx-auto flex max-w-3xl items-center gap-4">
				<span class="grid h-14 w-14 place-items-center rounded-2xl bg-brand-gradient text-white shadow-glow">
					<AppIcon name="user" :size="26" />
				</span>
				<div>
					<h1 class="text-xl font-bold tracking-tight md:text-2xl">Mon profil</h1>
					<p class="text-sm text-white/45">Préférences du catalogue et des sources légales</p>
				</div>
			</div>
		</header>

		<section class="mx-auto max-w-3xl space-y-6 px-4 py-6 md:px-6 lg:px-8">
			<div class="grid grid-cols-2 gap-3">
				<div v-for="stat in stats" :key="stat.label" class="surface p-4">
					<p class="text-2xl font-bold text-white">{{ stat.value }}</p>
					<p class="text-xs uppercase tracking-wide text-white/40">{{ stat.label }}</p>
				</div>
			</div>

			<div class="surface space-y-4 p-4">
				<h2 class="section-title">Préférences</h2>

				<label class="block space-y-1.5">
					<span class="text-xs uppercase tracking-wide text-white/40">Langue du catalogue</span>
					<select
						:value="settings.language"
						class="w-full rounded-xl border border-inkLine bg-ink px-3 py-2.5 text-sm text-white focus:border-brand focus:outline-none"
						@change="onLanguage"
					>
						<option v-for="language in LANGUAGES" :key="language.code" :value="language.code">
							{{ language.label }}
						</option>
					</select>
				</label>

				<label class="block space-y-1.5">
					<span class="text-xs uppercase tracking-wide text-white/40">Région des offres légales</span>
					<select
						:value="settings.region"
						class="w-full rounded-xl border border-inkLine bg-ink px-3 py-2.5 text-sm text-white focus:border-brand focus:outline-none"
						@change="onRegion"
					>
						<option v-for="region in REGIONS" :key="region.code" :value="region.code">
							{{ region.label }}
						</option>
					</select>
				</label>

				<label class="block space-y-1.5">
					<span class="text-xs uppercase tracking-wide text-white/40">Mode de performance</span>
					<select
						:value="settings.performanceMode || 'auto'"
						class="w-full rounded-xl border border-inkLine bg-ink px-3 py-2.5 text-sm text-white focus:border-brand focus:outline-none"
						@change="onPerformance"
					>
						<option value="auto">Automatique (recommandé)</option>
						<option value="low">Léger — téléphones anciens</option>
						<option value="high">Complet — appareils récents</option>
					</select>
				</label>

				<p class="text-[11px] text-white/35">
					La langue change les titres, synopsis et genres affichés. La région change les plateformes légales
					proposées (gratuit avec publicité, abonnement, location, achat). Le mode léger réduit les effets,
					la résolution des images et la mémoire utilisée.
				</p>
			</div>

			<div class="surface space-y-3 p-4">
				<h2 class="section-title">Mes données</h2>
				<div class="flex flex-wrap gap-2">
					<button type="button" class="btn btn-inverse px-3 py-2 text-xs" @click="clearHistory">
						<AppIcon name="trash" :size="14" />
						<span>Vider l'historique</span>
					</button>
					<button type="button" class="btn btn-inverse px-3 py-2 text-xs" @click="clearFavorites">
						<AppIcon name="trash" :size="14" />
						<span>Vider Ma liste</span>
					</button>
				</div>
				<p class="text-[11px] text-white/35">
					Favoris et historique sont stockés uniquement dans ton navigateur (localStorage). Pas de compte, pas
					d'envoi de données personnelles.
				</p>
			</div>

			<div class="surface space-y-3 p-4">
				<h2 class="section-title">Aide</h2>
				<button
					type="button"
					class="surface-soft flex w-full items-center gap-3 p-3.5 text-left transition-normal hover:border-white/20"
					@click="showDocs = true"
				>
					<span class="grid h-9 w-9 shrink-0 place-items-center rounded-lg bg-brand/15 text-brandSoft">
						<AppIcon name="info" :size="17" />
					</span>
					<span class="min-w-0 flex-1">
						<span class="block text-sm font-semibold text-white">Documentation</span>
						<span class="block text-[11px] text-white/45">
							Sources, lecture gratuite, téléchargements et confidentialité.
						</span>
					</span>
					<AppIcon name="chevronRight" :size="16" class="text-white/30" />
				</button>
			</div>

			<DocumentationModal v-if="showDocs" @close="showDocs = false" />

			<div class="surface space-y-2 p-4">
				<h2 class="section-title">État du serveur</h2>
				<div v-if="health" class="space-y-1 text-xs text-white/60">
					<p>API : <span class="text-emerald-400">en ligne</span> · uptime {{ health.uptime }} s</p>
					<p>Clé TMDB : {{ health.tmdbConfigured ? "configurée" : "manquante" }}</p>
					<p>Cache : {{ health.cache.entries }} entrées</p>
				</div>
				<p v-else-if="healthError" class="text-xs text-brandSoft">API hors ligne — {{ healthError }}</p>
				<p v-else class="text-xs text-white/40">Vérification…</p>
				<button type="button" class="btn btn-inverse w-fit px-3 py-2 text-xs" @click="checkServer">
					<AppIcon name="eye" :size="14" />
					<span>Revérifier</span>
				</button>
			</div>

			<div class="surface space-y-3 p-4">
				<div class="flex items-center justify-between gap-3">
					<div>
						<h2 class="section-title">Développeur</h2>
						<p class="text-[11px] text-white/40">
							Journal interne du serveur embarqué (utile en cas d'erreur).
						</p>
					</div>
					<button
						type="button"
						class="btn shrink-0 px-3 py-2 text-xs"
						:class="settings.developer ? 'btn-solid' : 'btn-inverse'"
						@click="setDeveloper(!settings.developer)"
					>
						<AppIcon name="eye" :size="14" />
						<span>{{ settings.developer ? "Désactiver" : "Activer" }}</span>
					</button>
				</div>

				<template v-if="settings.developer">
					<dl class="grid grid-cols-1 gap-1.5 text-xs sm:grid-cols-2">
						<div
							v-for="info in runtimeInfo"
							:key="info.label"
							class="flex min-w-0 items-baseline justify-between gap-2 rounded-lg bg-ink px-2.5 py-1.5"
						>
							<dt class="shrink-0 text-white/35">{{ info.label }}</dt>
							<dd class="truncate text-right text-white/75" :title="info.value">
								{{ info.value }}
							</dd>
						</div>
					</dl>

					<div class="flex flex-wrap gap-2">
						<button
							type="button"
							class="btn btn-inverse px-3 py-2 text-xs"
							:disabled="diagLoading"
							@click="refreshDiag"
						>
							<AppIcon name="eye" :size="14" />
							<span>{{ diagLoading ? "Chargement…" : "Rafraîchir" }}</span>
						</button>
						<button
							type="button"
							class="btn btn-inverse px-3 py-2 text-xs"
							:disabled="!diag?.log"
							@click="copyDiag"
						>
							<AppIcon name="info" :size="14" />
							<span>Copier le journal</span>
						</button>
					</div>

					<p v-if="diagError" class="text-xs text-brandSoft">{{ diagError }}</p>
					<pre
						v-if="diag?.log"
						class="max-h-72 overflow-auto whitespace-pre-wrap break-words rounded-xl border border-inkLine bg-black/60 p-3 text-[11px] leading-relaxed text-emerald-300/90"
					>{{ diag.log }}</pre>
					<p v-else-if="!diagError && !diagLoading" class="text-xs text-white/40">
						Aucune entrée pour l'instant.
					</p>
				</template>
			</div>

			<p class="text-center text-[11px] text-white/25">Movie Box · version web 1.0.0</p>
		</section>
	</div>
</template>
