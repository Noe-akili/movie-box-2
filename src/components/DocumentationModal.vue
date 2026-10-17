<script setup>
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import AppIcon from "./AppIcon.vue";

const emit = defineEmits(["close"]);

const STEPS = [
	"Explore l'accueil : les rails regroupent tendances, films, séries, anime et œuvres gratuites.",
	"Ouvre une fiche pour le synopsis, le casting, la bande-annonce, les saisons et les plateformes légales.",
	"Lance la lecture : œuvres libres de droits dans le lecteur intégré, bande-annonce pour les films protégés, ou lien vers une offre légale.",
];

const SECTIONS = [
	{
		id: "catalog",
		icon: "film",
		title: "Catalogue mondial",
		text: "Les métadonnées, affiches et bandes-annonces officielles proviennent de TMDB. Recherche, genres, langues et tri couvrent tout le catalogue des films, séries et animés.",
		bullets: [
			"Métadonnées dans 5 langues (FR, EN, ES, HI, KO, JA…)",
			"Filtres combinables : genre, langue, tri (popularité, note, date)",
			"Chargement infini : le catalogue se complète au fil du défilement",
		],
	},
	{
		id: "free",
		icon: "gift",
		title: "Films & séries gratuits",
		text: "Les œuvres libres de droits (domaine public ou licence Creative Commons) viennent d'Internet Archive et d'une source locale. Elles se lisent directement dans l'application, sans compte ni paiement.",
		bullets: [
			"Droits filtrés à la source via le champ licenseurl, affiché sur chaque fiche",
			"Collections : cinéma noir, films muets, animation, courts-métrages, archives Prelinger",
			"Lecteur HTML5 intégré avec choix de la qualité",
		],
	},
	{
		id: "series",
		icon: "tv",
		title: "Séries TV classiques",
		text: "Les archives multi-épisodes sont détectées automatiquement et transformées en épisodes numérotés et saisons navigables.",
		bullets: [
			"Motifs reconnus : S01E02, 1x02, « Épisode 3 », « 001. Titre », « Série - 001 - Titre »",
			"Dossiers « Season 4/ » convertis en saison 4",
			"Téléchargement épisode par épisode ou par saison entière",
		],
	},
	{
		id: "legal",
		icon: "link",
		title: "Où regarder légalement",
		text: "Pour les œuvres protégées, l'application référence les plateformes officielles de la région choisie en mettant en avant les offres gratuites financées par la publicité.",
		bullets: [
			"Plateformes : JustWatch, Pluto TV, Tubi, Plex, ARTE…",
			"Région configurable dans le profil",
			"Badges gratuit / abonnement / location / achat",
		],
	},
	{
		id: "library",
		icon: "bookmark",
		title: "Ma liste & historique",
		text: "Favoris, historique et reprise de lecture restent uniquement dans ton navigateur (localStorage). Aucun compte, aucune donnée envoyée.",
		bullets: [
			"Le bouton cœur ajoute ou retire une œuvre de Ma liste",
			"La lecture reprend là où tu t'étais arrêté",
			"Synchronisation impossible : tout est local par choix de confidentialité",
		],
	},
	{
		id: "downloads",
		icon: "download",
		title: "Téléchargements",
		text: "Depuis une fiche ou le lecteur, « Télécharger » ouvre un assistant : choisis la qualité, puis pour une série la saison, et sélectionne un ou plusieurs épisodes. Les fichiers sont conservés dans l'application pour la lecture hors ligne, ou enregistrés sur ton appareil.",
		bullets: [
			"Progression en direct : pourcentage, débit et temps restant",
			"Reprise après interruption grâce aux requêtes HTTP Range",
			"File d'attente limitée et stockage plafonné côté serveur",
		],
	},
	{
		id: "player",
		icon: "play",
		title: "Lecteur & qualité",
		text: "Le lecteur intégré diffuse les fichiers directement, en ligne ou hors ligne, avec sélection de la qualité disponible.",
		bullets: [
			"Qualités proposées selon les sources Internet Archive",
			"Plein écran et reprise de la position de lecture",
			"Lecture hors ligne depuis l'onglet Téléchargements",
		],
	},
	{
		id: "tips",
		icon: "star",
		title: "Astuces",
		text: "Quelques réflexes pour aller plus vite.",
		bullets: [
			"Recherche dès le premier mot : les suggestions s'affichent sous le champ",
			"Combiner langue + genre + tri affine précisément le catalogue",
			"Le menu « … » d'un téléchargement donne accès à pause, reprise et suppression",
			"Échap ferme les fenêtres ; un clic sur le fond aussi",
		],
	},
	{
		id: "privacy",
		icon: "eye",
		title: "Confidentialité",
		text: "Aucun compte n'est requis. Les préférences (langue, région, favoris, historique) sont stockées localement et ne quittent pas ton appareil.",
		bullets: [
			"Pas de cookie de suivi ni de profil publicitaire",
			"Les requêtes catalogue passent par le serveur Movie Box (clé TMDB jamais exposée)",
			"Polices et styles embarqués avec l'application : aucun appel à un CDN de polices",
			"Vider les données se fait depuis le profil",
		],
	},
];

const FAQ = [
	{
		q: "Pourquoi certains films ne se lisent-ils pas directement ?",
		a: "Seules les œuvres libres de droits (domaine public ou licences libres) peuvent être lues dans l'application. Pour les autres, Movie Box affiche la bande-annonce et les plateformes légales.",
	},
	{
		q: "La lecture d'un film gratuit ne démarre pas, que faire ?",
		a: "Internet Archive est parfois lent. Essaie une autre qualité, relance la lecture, ou reviens plus tard : la source est mutualisée et bénévole.",
	},
	{
		q: "Les téléchargements continuent-ils si je ferme l'application ?",
		a: "Le transfert est réalisé par le serveur. Un fichier partiellement reçu reprend automatiquement à l'octet suivant (HTTP Range) à la réouverture, ou via « Reprendre ».",
	},
	{
		q: "Où sont stockés mes favoris et mon historique ?",
		a: "Uniquement dans le stockage local de ton navigateur (localStorage). Aucun envoi vers un serveur, et vider ces données se fait depuis le profil.",
	},
	{
		q: "Comment regarder hors ligne ?",
		a: "Télécharge d'abord un film ou un épisode, puis ouvre l'onglet Téléchargements : le bouton « Regarder » lit le fichier local sans réseau.",
	},
	{
		q: "Puis-je changer la région des plateformes légales ?",
		a: "Oui. La région par défaut est la France et se modifie dans le profil : la liste « Où regarder » s'adapte à ta région.",
	},
];

const query = ref("");

const normalize = (value) =>
	String(value)
		.toLowerCase()
		.normalize("NFD")
		.replace(/\p{Diacritic}/gu, "");

const needle = computed(() => normalize(query.value.trim()));

const sectionHaystack = (section) =>
	[section.title, section.text, ...(section.bullets || [])].join(" ");

const visibleSections = computed(() => {
	if (!needle.value) return SECTIONS;
	return SECTIONS.filter((section) => normalize(sectionHaystack(section)).includes(needle.value));
});

const visibleFaq = computed(() => {
	if (!needle.value) return FAQ;
	return FAQ.filter((entry) =>
		normalize(`${entry.q} ${entry.a}`).includes(needle.value),
	);
});

const hasResults = computed(() => visibleSections.value.length > 0 || visibleFaq.value.length > 0);

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
				class="relative flex max-h-[90vh] w-full flex-col overflow-hidden rounded-t-3xl border border-inkLine bg-inkCard shadow-deck sm:max-w-xl sm:rounded-3xl"
				role="dialog"
				aria-modal="true"
				aria-label="Documentation Movie Box"
			>
				<header class="flex items-center gap-3 border-b border-white/5 px-4 py-3.5">
					<span class="grid h-9 w-9 shrink-0 place-items-center rounded-xl bg-brand/15 text-brandSoft">
						<AppIcon name="info" :size="18" />
					</span>
					<div class="min-w-0 flex-1">
						<h2 class="text-sm font-bold text-white">Documentation</h2>
						<p class="text-[11px] text-white/45">Tout savoir sur Movie Box · version 1.0.0</p>
					</div>
					<button type="button" class="icon-btn h-9 w-9" aria-label="Fermer" @click="emit('close')">
						<AppIcon name="close" :size="17" />
					</button>
				</header>

				<div class="min-h-0 flex-1 space-y-4 overflow-y-auto px-4 py-4">
					<label class="relative block">
						<span class="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-white/40">
							<AppIcon name="search" :size="16" />
						</span>
						<input
							v-model="query"
							type="search"
							class="w-full rounded-xl border border-inkLine bg-inkSoft/60 py-2.5 pl-9 pr-3 text-sm text-white placeholder:text-white/35 focus:border-brand/50 focus:outline-none"
							placeholder="Rechercher dans la documentation…"
							aria-label="Rechercher dans la documentation"
						/>
					</label>

					<div class="flex flex-wrap gap-1.5">
						<span class="badge badge-free">100 % gratuit</span>
						<span class="badge badge-brand">Sans compte</span>
						<span class="badge bg-white/10 text-white/70">Lecture hors ligne</span>
						<span class="badge bg-white/10 text-white/70">Films libres de droits</span>
					</div>

					<section v-if="!needle" class="surface-soft space-y-2.5 p-3.5">
						<div class="flex items-center gap-2">
							<AppIcon name="sparkles" :size="16" class="text-brandSoft" />
							<h3 class="text-sm font-semibold text-white">Démarrage rapide</h3>
						</div>
						<ol class="space-y-2">
							<li v-for="(step, index) in STEPS" :key="index" class="flex gap-2.5">
								<span
									class="grid h-5 w-5 shrink-0 place-items-center rounded-full bg-brand/20 text-[11px] font-bold text-brandSoft"
								>
									{{ index + 1 }}
								</span>
								<p class="text-xs leading-relaxed text-white/60">{{ step }}</p>
							</li>
						</ol>
					</section>

					<div class="space-y-3">
						<section
							v-for="section in visibleSections"
							:key="section.id"
							class="surface-soft space-y-2 p-3.5"
						>
							<div class="flex items-center gap-2">
								<AppIcon :name="section.icon" :size="16" class="text-brandSoft" />
								<h3 class="text-sm font-semibold text-white">{{ section.title }}</h3>
							</div>
							<p class="text-xs leading-relaxed text-white/55">{{ section.text }}</p>
							<ul v-if="section.bullets?.length" class="space-y-1.5">
								<li
									v-for="bullet in section.bullets"
									:key="bullet"
									class="flex gap-2 text-xs leading-relaxed text-white/45"
								>
									<AppIcon name="check" :size="13" class="mt-0.5 shrink-0 text-brand/70" />
									<span>{{ bullet }}</span>
								</li>
							</ul>
						</section>
					</div>

					<div v-if="visibleFaq.length" class="space-y-2">
						<h3 class="flex items-center gap-2 px-1 text-sm font-semibold text-white">
							<AppIcon name="message" :size="15" class="text-brandSoft" />
							Questions fréquentes
						</h3>
						<details
							v-for="entry in visibleFaq"
							:key="entry.q"
							class="group surface-soft overflow-hidden"
						>
							<summary
								class="flex cursor-pointer list-none items-center justify-between gap-3 px-3.5 py-3 text-xs font-semibold text-white/85 transition-normal hover:text-white [&::-webkit-details-marker]:hidden"
							>
								<span>{{ entry.q }}</span>
								<AppIcon
									name="chevronDown"
									:size="15"
									class="shrink-0 text-white/40 transition-transform group-open:rotate-180"
								/>
							</summary>
							<p class="px-3.5 pb-3 text-xs leading-relaxed text-white/50">{{ entry.a }}</p>
						</details>
					</div>

					<p
						v-if="!hasResults"
						class="rounded-xl border border-white/5 bg-white/[0.03] p-4 text-center text-xs text-white/45"
					>
						Aucun résultat pour « {{ query }} ».
					</p>

					<div class="flex items-start gap-2.5 rounded-xl border border-brand/20 bg-brand/5 p-3.5">
						<AppIcon name="shield" :size="16" class="mt-0.5 shrink-0 text-brandSoft" />
						<p class="text-[11px] leading-relaxed text-white/55">
							Movie Box n'héberge ni ne diffuse d'œuvre protégée par le droit d'auteur : l'application
							référence les contenus, lit les œuvres libres de droits et redirige vers les ayants droit
							pour le reste.
						</p>
					</div>
				</div>
			</div>
		</div>
	</Teleport>
</template>
