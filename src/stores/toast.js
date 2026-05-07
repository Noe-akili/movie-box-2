import { ref } from "vue";

const toasts = ref([]);

const removeToast = (id) => {
	toasts.value = toasts.value.filter((t) => t.id !== id);
};

const addToast = (type, message, options = {}) => {
	const id = `${Date.now()}_${Math.random().toString(16).slice(2)}`;
	const duration = options.duration ?? 3200;

	toasts.value.push({ id, type, message });

	window.setTimeout(() => removeToast(id), duration);
	return id;
};

export const toast = {
	success(message, options) {
		return addToast("success", message, options);
	},
	error(message, options) {
		return addToast("error", message, options);
	},
	info(message, options) {
		return addToast("info", message, options);
	},
};

export const useToasts = () => {
	return { toasts, removeToast };
};

