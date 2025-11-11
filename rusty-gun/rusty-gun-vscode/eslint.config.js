const js = require("@eslint/js");
const tseslint = require("@typescript-eslint/eslint-plugin");
const tsparser = require("@typescript-eslint/parser");

module.exports = [
  js.configs.recommended,
  {
    files: ["**/*.ts", "**/*.tsx"],
    languageOptions: {
      parser: tsparser,
      parserOptions: {
        ecmaVersion: 2020,
        sourceType: "module",
        project: "./tsconfig.json",
      },
      globals: {
        console: "readonly",
        setTimeout: "readonly",
        clearTimeout: "readonly",
        setInterval: "readonly",
        clearInterval: "readonly",
        WebSocket: "readonly",
        Buffer: "readonly",
        process: "readonly",
        __dirname: "readonly",
        __filename: "readonly",
        global: "readonly",
        Thenable: "readonly",
      },
    },
    plugins: {
      "@typescript-eslint": tseslint,
    },
    rules: {
      ...tseslint.configs.recommended.rules,
      "@typescript-eslint/no-unused-vars": ["warn", {
        argsIgnorePattern: "^_",
        varsIgnorePattern: "^_",
      }],
      "@typescript-eslint/no-explicit-any": "warn",
      "@typescript-eslint/explicit-function-return-type": "off",
      "@typescript-eslint/explicit-module-boundary-types": "off",
      "@typescript-eslint/no-inferrable-types": "off",
      "@typescript-eslint/no-non-null-assertion": "warn",
      "@typescript-eslint/no-var-requires": "error",
      "no-console": "off", // Allow console in VSCode extensions
      "no-debugger": "error",
      "no-duplicate-imports": "error",
      "no-unused-expressions": "error",
      "prefer-const": "error",
      "no-var": "error",
      "no-undef": "off", // TypeScript handles this
    },
  },
  {
    ignores: [
      "out/",
      "dist/",
      "node_modules/",
      "*.js",
      "*.d.ts",
      "*.js.map",
      "*.d.ts.map",
      "*.generated.*",
      "*.min.*",
      "*.config.js",
      "*.config.ts",
      "**/*.test.ts",
      "**/*.spec.ts",
      "*.md",
      "README.md",
      "package-lock.json",
      "*.vsix",
      "*.rs",
      "Cargo.toml",
      "Cargo.lock",
      ".vscode/",
      ".git/",
    ],
  },
];
