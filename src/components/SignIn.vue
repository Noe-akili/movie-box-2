<script setup>
import menu from "../images/menu.svg";
</script>

<template>
	<div class="flex items-center justify-end lg:order-3">
		<button
			type="button"
			class="flex items-center gap-3 font-bold text-white transition-normal hover:text-white/90 md:gap-5"
		>
			<span class="text-sm md:text-base">Sign in</span>
			<span
				class="grid h-9 w-9 place-items-center rounded-full bg-rose shadow-soft ring-1 ring-white/10 md:h-10 md:w-10"
			>
				<img :src="menu" alt="" aria-hidden="true" class="h-4 w-4" />
			</span>
		</button>
	</div>
</template>

