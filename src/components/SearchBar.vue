<script setup>
import IconSearch from "./icons/IconSearch.vue";

const props = defineProps({
	modelValue: { type: String, default: "" },
});

const emit = defineEmits(["update:modelValue", "submit"]);

const onSubmit = (e) => {
	e.preventDefault();
	emit("submit");
};
</script>

<template>
	<form
		@submit="onSubmit"
		class="glass-field col-span-2 flex items-center justify-between gap-2 rounded-lg px-3 py-2 text-white focus-within:border-white/40 focus-within:ring-2 focus-within:ring-white/25 md:px-4 lg:flex-1 lg:max-w-[540px]"
	>
		<input
			type="text"
			placeholder="What do you want to watch?"
			aria-label="Search movies"
			class="w-full bg-transparent text-sm placeholder:text-white/80 focus:outline-none md:text-base"
			:value="props.modelValue"
			@input="emit('update:modelValue', $event.target.value)"
		/>
		<button
			type="submit"
			class="rounded-md p-2 text-white/90 transition-normal hover:bg-white/10 hover:text-white focus:outline-none focus-visible:ring-2 focus-visible:ring-white/30"
			aria-label="Search"
		>
			<IconSearch />
		</button>
	</form>
</template>

