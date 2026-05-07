<script setup>
import { computed } from "vue";
import { useToasts } from "../stores/toast";

const { toasts, removeToast } = useToasts();

const orderedToasts = computed(() => [...toasts.value].slice(-3));
</script>

<template>
	<div
		class="fixed inset-x-0 bottom-4 z-50 flex justify-center px-4 pointer-events-none"
	>
		<div class="w-full max-w-md space-y-2">
			<TransitionGroup name="toast" tag="div" class="space-y-2">
				<div
					v-for="t in orderedToasts"
					:key="t.id"
					class="pointer-events-auto flex items-start justify-between gap-3 rounded-xl border border-black/10 bg-white px-4 py-3 shadow-card"
				>
					<div class="flex items-start gap-3">
						<span
							class="mt-0.5 inline-flex h-2.5 w-2.5 shrink-0 rounded-full"
							:class="
								t.type === 'success'
									? 'bg-emerald-500'
									: t.type === 'error'
										? 'bg-rose'
										: 'bg-sky-500'
							"
						></span>
						<p class="text-sm font-medium text-darkestGray">
							{{ t.message }}
						</p>
					</div>
					<button
						type="button"
						class="rounded-md px-2 py-1 text-sm text-darkerGray transition-normal hover:bg-black/5 hover:text-darkestGray"
						@click="removeToast(t.id)"
						aria-label="Close notification"
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

