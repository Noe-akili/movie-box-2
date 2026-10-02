<script setup>
import { computed } from "vue";
import { useToasts } from "../stores/toast";

const { toasts, removeToast } = useToasts();

const orderedToasts = computed(() => [...toasts.value].slice(-3));

const dotClass = (type) =>
	type === "success" ? "bg-emerald-400" : type === "error" ? "bg-brandSoft" : "bg-sky-400";
</script>

<template>
	<div class="pointer-events-none fixed inset-x-0 bottom-24 z-50 flex justify-center px-4 lg:bottom-8">
		<div class="w-full max-w-md space-y-2">
			<TransitionGroup name="toast" tag="div" class="space-y-2">
				<div
					v-for="t in orderedToasts"
					:key="t.id"
					class="pointer-events-auto flex items-start justify-between gap-3 rounded-xl border border-white/10 bg-inkRaised/95 px-4 py-3 shadow-deck backdrop-blur"
				>
					<div class="flex items-start gap-3">
						<span class="mt-1 inline-flex h-2.5 w-2.5 shrink-0 rounded-full" :class="dotClass(t.type)"></span>
						<p class="text-sm font-medium text-white">{{ t.message }}</p>
					</div>
					<button
						type="button"
						class="rounded-md px-2 py-1 text-sm text-white/50 transition-normal hover:bg-white/10 hover:text-white"
						aria-label="Fermer la notification"
						@click="removeToast(t.id)"
					>
						×
					</button>
				</div>
			</TransitionGroup>
		</div>
	</div>
</template>

<style scoped>
.toast-enter-active,
.toast-leave-active {
	transition: transform 160ms ease, opacity 160ms ease;
}

.toast-enter-from,
.toast-leave-to {
	opacity: 0;
	transform: translateY(10px);
}
</style>

