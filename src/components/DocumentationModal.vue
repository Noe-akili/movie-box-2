<script setup>
import { onBeforeUnmount, onMounted } from "vue";
import AppIcon from "./AppIcon.vue";

const emit = defineEmits(["close"]);

const SECTIONS = [
	{
		icon: "film",
		title: "Catalogue mondial",
		text: "Les métadonnées, affiches et bandes-annonces officielles viennent de TMDB : recherche, genres, langues et tri sur tout le catalogue des films, séries et animés.",
	},
	{
		icon: "gift",
		title: "Contenus gratuits",
		text: "Les œuvres libres de droits (domaine public ou licence Creative Commons) proviennent d'Internet Archive et de la source MovieBox locale. Elles se lisent directement dans l'application, sans compte ni paiement.",
	},
	{
		icon: "download",
		title: "Téléchargements",
		text: "Depuis une fiche ou le lecteur, « Télécharger » ouvre un assistant : choisis la qualité, puis pour une série la saison, et sélectionne un ou plusieurs épisodes. Le serveur stocke les fichiers pour une lecture hors ligne ou un enregistrement sur l'appareil.",
	},
	{
		icon: "link",
		title: "Où regarder légalement",
		text: "Pour les œuvres protégées, l'application référence les plateformes officielles (JustWatch, Pluto TV, Tubi, Plex, ARTE…) en mettant en avant les offres gratuites financées par la publicité.",
	},
	{
		icon: "bookmark",
		title: "Ma liste & confidentialité",
		text: "Favoris, historique et préférences restent stockés uniquement dans ton navigateur (localStorage). Aucun compte n'est requis et aucune donnée personnelle n'est envoyée.",
	},
];

const onKeydown = (event) => {
	if (event.key === "Escape") emit("close");
};

onMounted(() => {
	document.body.style.overflow = "hidden";
	window.addEventListener("keydown", onKeydown);
});

onBeforeUnmount(() => {
	document.body.style.overflow = "";
	window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
	<Teleport to="body">
		<div class="fixed inset-0 z-[60] flex items-end justify-center sm:items-center">
			<button
				type="button"
				class="absolute inset-0 bg-black/70 backdrop-blur-sm"
				aria-label="Fermer"
				@click="emit('close')"
			></button>

			<div
				class="relative flex max-h-[88vh] w-full flex-col overflow-hidden rounded-t-3xl border border-inkLine bg-inkCard shadow-deck sm:max-w-lg sm:rounded-3xl"
				role="dialog"
				aria-modal="true"
			>
				<header class="flex items-center gap-3 border-b border-white/5 px-4 py-3.5">
					<span class="grid h-9 w-9 shrink-0 place-items-center rounded-xl bg-brand/15 text-brandSoft">
						<AppIcon name="info" :size="18" />
					</span>
					<div class="min-w-0 flex-1">
						<h2 class="text-sm font-bold text-white">Documentation</h2>
						<p class="text-[11px] text-white/45">À propos de Movie Box</p>
					</div>
					<button type="button" class="icon-btn h-9 w-9" aria-label="Fermer" @click="emit('close')">
						<AppIcon name="close" :size="17" />
					</button>
				</header>

				<div class="min-h-0 flex-1 space-y-3 overflow-y-auto px-4 py-4">
					<div v-for="section in SECTIONS" :key="section.title" class="surface-soft space-y-1.5 p-3.5">
						<div class="flex items-center gap-2">
							<AppIcon :name="section.icon" :size="16" class="text-brandSoft" />
							<h3 class="text-sm font-semibold text-white">{{ section.title }}</h3>
						</div>
						<p class="text-xs leading-relaxed text-white/55">{{ section.text }}</p>
					</div>

					<p class="rounded-xl border border-white/5 bg-white/[0.03] p-3 text-[11px] leading-relaxed text-white/45">
						Movie Box n'héberge ni ne diffuse d'œuvre protégée par le droit d'auteur : l'application référence
						les contenus, lit les œuvres libres de droits et redirige vers les ayants droit pour le reste.
					</p>
				</div>
			</div>
		</div>
	</Teleport>
</template>
