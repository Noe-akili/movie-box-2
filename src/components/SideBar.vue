<script setup>
import Logo from "./Logo.vue";
import home from "../images/home.svg";
import projector from "../images/projector.svg";
import calendar from "../images/calendar.svg";
import tvShow from "../images/tv_show.svg";
import logout from "../images/logout.svg";

const props = defineProps({
	isMenuOpen: { type: Boolean, default: false },
});

const links = [
	{ path: "/", name: "Home", icon: home, active: false },
	{ path: "/", name: "Movies", icon: projector, active: true },
	{ path: "/", name: "TV Shows", icon: tvShow, active: false },
	{ path: "/", name: "Upcoming", icon: calendar, active: false },
];
</script>

<template>
	<aside
		class="fixed md:static bg-white border-black/30 border-r flex-col justify-between h-screen py-8 w-[230px] rounded-r-[45px] text-[#666] md:flex transition-all ease-in-out duration-500 md:translate-x-0"
		:class="props.isMenuOpen ? 'translate-x-0' : '-translate-x-full'"
	>
		<div class="px-4 hidden md:block">
			<Logo color="text-black" />
		</div>
		<ul class="font-medium">
			<li v-for="(link, i) in links" :key="i">
				<RouterLink
					:to="link.path"
					class="block py-5 transition-normal hover:text-rose hover:bg-lightRose pl-10 hover:border-rose hover:border-r-[6px]"
					:class="
						link.active
							? 'text-rose bg-lightRose border-rose border-r-[6px]'
							: ''
					"
				>
					<div class="flex items-center gap-4">
						<img :src="link.icon" :alt="link.name" />
						<span class="text-lg">{{ link.name }}</span>
					</div>
				</RouterLink>
			</li>
		</ul>
		<div class="px-4 hidden md:block">
			<div class="bg-lightRose border border-rose px-4 py-8 rounded-[20px] space-y-3">
				<p class="text-sm font-medium">
					Play movie quizzes and earn free tickets
				</p>
				<p class="text-xs">50k people are playing now</p>
				<button
					type="button"
					class="text-xs px-5 py-2 bg-lightRose text-rose font-medium block mx-auto rounded-3xl hover:bg-rose hover:text-white transition-normal"
				>
					Start playing
				</button>
			</div>
		</div>
		<div class="pl-10 mt-4">
			<div class="flex items-center gap-4">
				<img :src="logout" alt="log out" />
				<span>Log out</span>
			</div>
		</div>
	</aside>
</template>

