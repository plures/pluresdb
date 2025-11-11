import js from "@eslint/js";
import importPlugin from "eslint-plugin-import";
import tsPlugin from "@typescript-eslint/eslint-plugin";
import tsParser from "@typescript-eslint/parser";
import prettierConfig from "eslint-config-prettier";

const tsRecommendedRules = tsPlugin.configs.recommended?.rules ?? {};

export default [
  {
    ignores: [
      "dist/**",
      "node_modules/**",
      "target/**",
      "web/**",
      "docs/**",
      "packaging/**",
      "demo/**",
      "coverage/**",
      "scripts/**",
      "scripts/run-tests.ts",
      "legacy/tests/**",
      "legacy/core/**",
      "legacy/http/**",
      "legacy/logic/**",
      "legacy/network/**",
      "legacy/storage/**",
      "legacy/util/**",
      "legacy/vector/**",
      "legacy/index.ts",
    ],
  },
  js.configs.recommended,
  {
    files: ["**/*.{ts,tsx,js,jsx}"],
    languageOptions: {
      parser: tsParser,
      parserOptions: {
        ecmaVersion: 2022,
        sourceType: "module",
      },
      globals: {
        console: "readonly",
        process: "readonly",
        require: "readonly",
        module: "readonly",
        exports: "readonly",
        __dirname: "readonly",
        fetch: "readonly",
        setTimeout: "readonly",
        clearTimeout: "readonly",
        performance: "readonly",
        TextDecoder: "readonly",
        AbortSignal: "readonly",
        WebSocket: "readonly",
        Deno: "readonly",
        crypto: "readonly",
      },
    },
    plugins: {
      "@typescript-eslint": tsPlugin,
      import: importPlugin,
    },
    rules: {
      ...tsRecommendedRules,
      "import/order": [
        "warn",
        {
          groups: [
            ["builtin", "external"],
            ["internal"],
            ["parent", "sibling", "index"],
          ],
          "newlines-between": "always",
        },
      ],
      "import/no-unresolved": "off",
      "@typescript-eslint/no-explicit-any": "off",
      "@typescript-eslint/no-unused-vars": [
        "warn",
        {
          argsIgnorePattern: "^_",
          varsIgnorePattern: "^_",
        },
      ],
      "@typescript-eslint/no-require-imports": "off",
      "no-case-declarations": "off",
      "no-undef": "off",
    },
  },
  prettierConfig,
];
