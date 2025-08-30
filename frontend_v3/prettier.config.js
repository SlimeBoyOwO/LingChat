/**
 * @see https://prettier.io/docs/configuration
 * @type {import("prettier").Config}
 */
const config = {
    printWidth: 120,
    tabWidth: 4,
    useTabs: false,
    arrowParens: "avoid",
    trailingComma: "none",
    endOfLine: "lf",
    importOrder: ["^@core/(.*)$", "", "^@server/(.*)$", "", "^@ui/(.*)$", "", "^[./]"],
    importOrderTypeScriptVersion: "5.0.0",
    plugins: ["@ianvs/prettier-plugin-sort-imports", "prettier-plugin-tailwindcss"]
};

export default config;
