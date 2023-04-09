const themeSwapper = require("tailwindcss-theme-swapper")
const colors = require("tailwindcss/colors")

/** @type {<K, V>(obj: Record<K, V>) => Record<K, V>} */
const reverseObj = (obj) => {
  const entries = Object.entries(obj)
  const reversed = {}
  for (let i = 0; i < entries.length; i++) {
    reversed[entries[i][0]] = entries[entries.length - i - 1][1]
  }
  return reversed
}

/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./index.html",
    "./**/*.{ts,tsx}"
  ],
  theme: {
    extend: {},
  },
  plugins: [
    themeSwapper({
      themes: [
        {
          name: "base",
          selectors: [":root"],
          theme: {
            colors: {
              primary: colors.sky[800],
              "fg-primary": colors.white,
              "primary-hover": colors.sky[700],
              "primary-highlight": colors.sky[800],

              secondary: colors.gray[500],
              "fg-secondary": colors.gray[100],
              "secondary-hover": colors.gray[400],

              black: colors.black,
              white: colors.white,
              slate: colors.slate,
              gray: colors.gray,
              zinc: colors.zinc,
              neutral: colors.neutral,
              stone: colors.stone,
              red: colors.red,
              orange: colors.orange,
              amber: colors.amber,
              yellow: colors.yellow,
              lime: colors.lime,
              green: colors.green,
              emerald: colors.emerald,
              teal: colors.teal,
              cyan: colors.cyan,
              sky: colors.sky,
              blue: colors.blue,
              indigo: colors.indigo,
              violet: colors.violet,
              purple: colors.purple,
              fuchsia: colors.fuchsia,
              pink: colors.pink,
              rose: colors.rose,
            },
          }
        },
        {
          name: "dark",
          selectors: [".dark", ".vscode-dark", ".vscode-high-contrast"],
          mediaQuery: '@media (prefers-color-scheme: dark)',
          theme: {
            colors: {
              primary: colors.sky[600],
              "primary-hover": colors.sky[700],
              "fg-primary": colors.white,
              "primary-highlight": colors.sky[500],

              secondary: colors.gray[500],
              "fg-secondary": colors.gray[100],
              "secondary-hover": colors.gray[600],

              black: colors.white,
              white: colors.black,
              slate: reverseObj(colors.slate),
              gray: reverseObj(colors.gray),
              zinc: reverseObj(colors.zinc),
              neutral: reverseObj(colors.neutral),
              stone: reverseObj(colors.stone),
              red: reverseObj(colors.red),
              orange: reverseObj(colors.orange),
              amber: reverseObj(colors.amber),
              yellow: reverseObj(colors.yellow),
              lime: reverseObj(colors.lime),
              green: reverseObj(colors.green),
              emerald: reverseObj(colors.emerald),
              teal: reverseObj(colors.teal),
              cyan: reverseObj(colors.cyan),
              sky: reverseObj(colors.sky),
              blue: reverseObj(colors.blue),
              indigo: reverseObj(colors.indigo),
              violet: reverseObj(colors.violet),
              purple: reverseObj(colors.purple),
              fuchsia: reverseObj(colors.fuchsia),
              pink: reverseObj(colors.pink),
              rose: reverseObj(colors.rose),
            },
          },
        }
      ]
    }),
  ],
}
