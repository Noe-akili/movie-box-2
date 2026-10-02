<script setup>
import { computed, onMounted, ref } from "vue";
import { fetchHealth } from "../api/client";
import AppIcon from "../components/AppIcon.vue";
import { useLibrary } from "../stores/library";
import { LANGUAGES, REGIONS, updateSettings, useSettings } from "../stores/settings";
import { toast } from "../stores/toast";

const settings = useSettings();
const { favorites, history, clearFavorites, clearHistory } = useLibrary();

const health = ref(null);
const healthError = ref(null);

const stats = computed(() => [
	{ label: "Favoris", value: favorites.value.length },
	{ label: "Historique", value: history.value.length },
]);

const onLanguage = (event) => {
	updateSettings({ language: event.target.value });
	toast.success("Langue du catalogue mise à jour");
};

const onRegion = (event) => {
	updateSettings({ region: event.target.value });
	toast.success("Région des offres légales mise à jour");
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

onMounted(checkServer);
</script>


<template>
	<div class="pb-nav">
		<header class="border-b border-white/5 bg-inkSoft/60 px-4 pb-6 pt-20 md:px-6 md:pt-24 lg:px-8">
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

				<p class="text-[11px] text-white/35">
					La langue change les titres, synopsis et genres affichés. La région change les plateformes légales
					proposées (gratuit avec publicité, abonnement, location, achat).
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
				<h2 class="section-title">Sources &amp; légalité</h2>
				<ul class="space-y-2 text-sm text-white/70">
					<li class="flex gap-2">
						<AppIcon name="film" :size="16" class="mt-0.5 text-white/40" />
						<span>
							<strong class="text-white">TMDB</strong> — métadonnées, affiches et bandes-annonces officielles de tout
							le catalogue mondial.
						</span>
					</li>
					<li class="flex gap-2">
						<AppIcon name="archive" :size="16" class="mt-0.5 text-emerald-400" />
						<span>
							<strong class="text-white">Internet Archive</strong> — films du domaine public ou diffusés librement :
							lecture directe et gratuite dans Movie Box.
						</span>
					</li>
					<li class="flex gap-2">
						<AppIcon name="link" :size="16" class="mt-0.5 text-white/40" />
						<span>
							<strong class="text-white">Plateformes légales</strong> — JustWatch, Pluto TV, Tubi, Plex, ARTE… dont
							les offres gratuites avec publicité.
						</span>
					</li>
				</ul>

				<p class="rounded-xl border border-white/5 bg-white/[0.03] p-3 text-[11px] text-white/45">
					Movie Box n'héberge ni ne diffuse d'œuvre protégée par le droit d'auteur : l'application référence les
					contenus et redirige vers les ayants droit, ce qui garantit sa conformité et sa pérennité.
				</p>
			</div>

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

			<p class="text-center text-[11px] text-white/25">Movie Box · version web 1.0.0</p>
		</section>
	</div>
</template>
