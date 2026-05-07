import { ref } from "vue";

const isLoading = ref(false);

export const useLoading = () => isLoading;

export const useLoadingUpdate = () => {
	return (value) => {
		isLoading.value = Boolean(value);
	};
};

