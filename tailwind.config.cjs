/** @type {import('tailwindcss').Config} */
module.exports = {
	content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx,vue}"],
	theme: {
		container: {
			center: true,
			padding: {
				DEFAULT: "1rem",
				md: "2rem",
				xl: "3rem",
			},
		},
		screens: {
			sm: "512px",
			md: "768px",
			lg: "1200px",
			xl: "1440px",
		},
		extend: {
			colors: {
				// Palette sombre de l'application (style MovieBox)
				ink: "#07070B",
				inkSoft: "#0E0E15",
				inkCard: "#16161F",
				inkRaised: "#1E1E2A",
				inkLine: "#2A2A38",
				brand: "#BE123C",
				brandSoft: "#F43F5E",
				gold: "#F5C518",
				// Anciens jetons conserves pour les composants historiques
				gray: "#9CA3AF",
				darkGray: "#666666",
				darkerLightGray: "#9CA3AF",
				darkerGray: "#6B7280",
				darkestGray: "#111827",
				rose: "#BE123C",
				lightRose: "#BE123C1A",
				lightGray: "#C7C7C7",
				lightestGray: "#D1D5DB",
				extraLightGray: "#E8E8E8",
				glassGray: "rgba(243, 244, 246, 0.50)",
				borderGray: "#F8E7EB",
			},
			boxShadow: {
				card: "0 18px 45px -20px rgba(17, 24, 39, 0.35)",
				soft: "0 10px 25px -18px rgba(17, 24, 39, 0.25)",
				glow: "0 12px 40px -12px rgba(190, 18, 60, 0.55)",
				deck: "0 18px 40px -22px rgba(0, 0, 0, 0.9)",
			},
			backgroundImage: {
				"hero-gradient":
					"linear-gradient(90deg, rgba(0,0,0,0.85) 0%, rgba(0,0,0,0.55) 45%, rgba(0,0,0,0.20) 100%)",
				"hero-scrim":
					"linear-gradient(180deg, rgba(7,7,11,0.5) 0%, rgba(7,7,11,0.15) 32%, #07070B 100%)",
				"card-scrim": "linear-gradient(180deg, rgba(7,7,11,0) 45%, rgba(7,7,11,0.92) 100%)",
				"brand-gradient": "linear-gradient(120deg, #BE123C 0%, #F43F5E 55%, #FB7185 100%)",
			},
			fontFamily: {
				dmSans: ["DM Sans", "sans-serif"],
				poppins: ["Poppins", "sans-serif"],
			},
			keyframes: {
				shimmer: {
					"0%": { backgroundPosition: "-500px 0" },
					"100%": { backgroundPosition: "500px 0" },
				},
				fadeUp: {
					"0%": { opacity: "0", transform: "translateY(10px)" },
					"100%": { opacity: "1", transform: "translateY(0)" },
				},
			},
			animation: {
				shimmer: "shimmer 1.4s linear infinite",
				fadeUp: "fadeUp 0.35s ease-out both",
			},
		},
	},
	plugins: [],
};
