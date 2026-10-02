<script setup>
import AppIcon from "./AppIcon.vue";

const props = defineProps({
	icon: { type: String, default: "info" },
	title: { type: String, default: "Rien à afficher" },
	description: { type: String, default: "" },
	actionLabel: { type: String, default: "" },
});

const emit = defineEmits(["action"]);
</script>

<template>
	<div class="mx-auto flex max-w-md flex-col items-center gap-3 px-6 py-14 text-center">
		<div class="grid h-14 w-14 place-items-center rounded-2xl bg-inkCard text-white/50 ring-1 ring-white/5">
			<AppIcon :name="props.icon" :size="26" />
		</div>
		<h2 class="text-base font-semibold text-white">{{ props.title }}</h2>
		<p v-if="props.description" class="text-sm text-white/45">{{ props.description }}</p>
		<button v-if="props.actionLabel" type="button" class="btn btn-solid mt-1" @click="emit('action')">
			{{ props.actionLabel }}
		</button>
	</div>
</template>
