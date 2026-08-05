import js from "@eslint/js";
import tseslint from "typescript-eslint";
import reactHooks from "eslint-plugin-react-hooks";
import jsxA11y from "eslint-plugin-jsx-a11y";
import globals from "globals";

// Flat config. Two rule families carry the M9 quality floor:
//   - react-hooks: errors. The ref-shadow pattern (App.tsx) is load-bearing and
//     a missed dep is exactly the class of silent-failure bug this milestone
//     fixes. The one legitimate exception is disabled inline (App.tsx mount
//     effect).
//   - jsx-a11y: warnings for now. It flags real pre-existing gaps (keyboard-dead
//     editor tabs, missing aria-pressed) that Track E §8.2 fixes in Phase 4;
//     that phase promotes these to errors. Warnings keep them visible without
//     blocking the incremental phases before the fixes land.
export default tseslint.config(
  {
    ignores: [
      "dist",
      "engine",
      "e2e",
      "src/lang/parser.js",
      "src/lang/parser.terms.js",
      "*.config.ts",
      "*.config.js",
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    files: ["src/**/*.{ts,tsx}"],
    languageOptions: {
      globals: { ...globals.browser, ...globals.worker },
    },
    plugins: {
      "react-hooks": reactHooks,
      "jsx-a11y": jsxA11y,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      ...jsxA11y.flatConfigs.recommended.rules,
      // Demote a11y to warnings until Phase 4 lands the fixes (see header).
      ...Object.fromEntries(
        Object.keys(jsxA11y.flatConfigs.recommended.rules).map((r) => [
          r,
          "warn",
        ]),
      ),
      // Worker/websocket message plumbing legitimately passes values whose shape
      // is only known at the boundary; the codebase narrows them explicitly.
      "@typescript-eslint/no-explicit-any": "off",
    },
  },
);
