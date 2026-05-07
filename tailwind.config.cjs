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
			},
			backgroundImage: {
				"hero-gradient":
					"linear-gradient(90deg, rgba(0,0,0,0.85) 0%, rgba(0,0,0,0.55) 45%, rgba(0,0,0,0.20) 100%)",
			},
			fontFamily: {
				dmSans: ["DM Sans", "sans-serif"],
				poppins: ["Poppins", "sans-serif"],
			},
		},
	},
	plugins: [],
};
