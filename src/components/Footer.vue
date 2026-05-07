<script setup>
const year = new Date().getFullYear();
</script>

<template>
	<footer class="py-10 font-bold text-darkestGray md:py-12">
		<div class="container flex flex-col items-center space-y-5 lg:space-y-6">
			<ul class="flex space-x-6 md:space-x-8 lg:space-x-10">
				<li class="cursor-pointer page-link">Facebook</li>
				<li class="cursor-pointer page-link">Instagram</li>
				<li class="cursor-pointer page-link">Twitter</li>
				<li class="cursor-pointer page-link">YouTube</li>
			</ul>
			<ul class="flex flex-wrap justify-center gap-4 gap-y-2 text-sm lg:text-[1rem]">
				<li>Conditions of Use</li>
				<li>Privacy &amp; Policy</li>
				<li>Press Room</li>
			</ul>
			<p class="text-darkerGray text-sm lg:text-[1rem]">
				© {{ year }} MovieBox by Devsheunn.
			</p>
		</div>
	</footer>
</template>

