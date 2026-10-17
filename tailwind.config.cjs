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
				// Palette « cinema » : noirs profonds neutres + accent vert signature.
				ink: "#07070A",
				inkDeep: "#040406",
				inkSoft: "#101015",
				inkCard: "#16161D",
				inkRaised: "#20202A",
				inkLine: "#2A2A36",
				brand: "#22C55E",
				brandSoft: "#4ADE80",
				brandBlue: "#22D3EE",
				brandDeep: "#15803D",
				brandTeal: "#2DD4BF",
				gold: "#F5C518",
				// Anciens jetons conserves pour les composants historiques
				gray: "#9CA3AF",
				darkGray: "#666666",
				darkerLightGray: "#9CA3AF",
				darkerGray: "#6B7280",
				darkestGray: "#111827",
				rose: "#22C55E",
				lightRose: "#22C55E1A",
				lightGray: "#C7C7C7",
				lightestGray: "#D1D5DB",
				extraLightGray: "#E8E8E8",
				glassGray: "rgba(243, 244, 246, 0.50)",
				borderGray: "#2C2E34",
			},
			boxShadow: {
				card: "0 18px 45px -20px rgba(0, 0, 0, 0.55)",
				soft: "0 10px 25px -18px rgba(0, 0, 0, 0.45)",
				glow: "0 12px 40px -12px rgba(34, 197, 94, 0.45)",
				glowSoft: "0 10px 30px -14px rgba(34, 197, 94, 0.55)",
				deck: "0 18px 40px -22px rgba(0, 0, 0, 0.9)",
				poster: "0 22px 45px -26px rgba(0, 0, 0, 0.95)",
				hover: "0 28px 55px -20px rgba(0, 0, 0, 0.92), 0 0 0 1px rgba(255, 255, 255, 0.09)",
				nav: "0 12px 34px -16px rgba(0, 0, 0, 0.85)",
				topbar: "0 14px 36px -22px rgba(0, 0, 0, 1)",
			},
			backgroundImage: {
				"hero-gradient":
					"linear-gradient(77deg, rgba(0,0,0,0.94) 0%, rgba(0,0,0,0.62) 38%, rgba(0,0,0,0.08) 78%, transparent 100%)",
				"hero-scrim":
					"linear-gradient(180deg, rgba(7,7,10,0.5) 0%, rgba(7,7,10,0.05) 30%, rgba(7,7,10,0.55) 72%, #07070A 100%)",
				"card-scrim": "linear-gradient(180deg, rgba(7,7,10,0) 42%, rgba(7,7,10,0.94) 100%)",
				// Boutons principaux : vert lumineux vers menthe, signature MovieBox.
				"brand-gradient": "linear-gradient(120deg, #22C55E 0%, #34D399 100%)",
			},
			fontFamily: {
				// Repli système complet : le premier affichage reste net même si la
				// webfont n'est pas encore téléchargée (et hors ligne).
				dmSans: [
					"DM Sans",
					"system-ui",
					"-apple-system",
					"Segoe UI",
					"Roboto",
					"Helvetica Neue",
					"Arial",
					"sans-serif",
				],
				poppins: [
					"Poppins",
					"system-ui",
					"-apple-system",
					"Segoe UI",
					"Roboto",
					"sans-serif",
				],
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
				kenburns: {
					"0%": { transform: "scale(1.02)" },
					"100%": { transform: "scale(1.14)" },
				},
				riseIn: {
					"0%": { opacity: "0", transform: "translateY(18px)" },
					"100%": { opacity: "1", transform: "translateY(0)" },
				},
				heroProgress: {
					"0%": { width: "0%" },
					"100%": { width: "100%" },
				},
			},
			animation: {
				shimmer: "shimmer 1.4s linear infinite",
				fadeUp: "fadeUp 0.35s ease-out both",
				kenburns: "kenburns 14s ease-out both",
				riseIn: "riseIn 0.5s cubic-bezier(0.22, 1, 0.36, 1) both",
				heroProgress: "heroProgress 8s linear forwards",
			},
		},
	},
	plugins: [],
};
