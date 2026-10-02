export class ApiError extends Error {
	constructor(status, message, meta = {}) {
		super(message);
		this.name = "ApiError";
		this.status = status;
		this.meta = meta;
	}
}

export const badRequest = (message) => new ApiError(400, message);
export const notFound = (message = "Ressource introuvable") => new ApiError(404, message);

/** Enveloppe un handler async pour transmettre les erreurs a Express. */
export const asyncHandler = (handler) => (req, res, next) =>
	Promise.resolve(handler(req, res, next)).catch(next);
