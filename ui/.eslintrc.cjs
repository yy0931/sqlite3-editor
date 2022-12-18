const path = require("path")

module.exports = {
    extends: [],
    parser: '@typescript-eslint/parser',
    plugins: ['@typescript-eslint'],
    root: true,
    parserOptions: {
        project: path.join(__dirname, "tsconfig.json"),
    },
    rules: {
        "@typescript-eslint/no-floating-promises": "warn"
    },
    overrides: [
        {
            files: ['*.ts', '*.tsx']
        }
    ]
}
