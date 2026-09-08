/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{vue,ts}"],
  theme: {
    // Colours live in CSS custom properties so light/dark is one swap,
    // and so "state colour" stays a small, named vocabulary.
    colors: {
      transparent: "transparent",
      current: "currentColor",
      window: "var(--window)",
      surface: "var(--surface)",
      raised: "var(--raised)",
      sunken: "var(--sunken)",
      hairline: "var(--hairline)",
      edge: "var(--edge)",
      ink: "var(--ink)",
      muted: "var(--muted)",
      faint: "var(--faint)",
      accent: "var(--accent)",
      "accent-ink": "var(--accent-ink)",
      working: "var(--working)",
      waiting: "var(--waiting)",
      resting: "var(--resting)",
      danger: "var(--danger)",
    },
    fontFamily: {
      sans: ["var(--font-sans)"],
      mono: ["var(--font-mono)"],
    },
    fontSize: {
      // A compact Mac-app scale; 13px is the body size, not 16.
      "2xs": ["10px", "14px"],
      xs: ["11px", "15px"],
      sm: ["12px", "17px"],
      base: ["13px", "19px"],
      md: ["14px", "20px"],
      lg: ["17px", "23px"],
      xl: ["22px", "28px"],
    },
    borderRadius: {
      none: "0",
      sm: "4px",
      DEFAULT: "6px",
      md: "8px",
      lg: "10px",
      full: "9999px",
    },
    extend: {
      spacing: { titlebar: "38px" },
      transitionDuration: { DEFAULT: "120ms" },
    },
  },
  plugins: [],
};
