import { describe, it, expect } from "vitest";
import { createRoot } from "react-dom/client";
import { act } from "react";

// Proves the jsdom vitest project is wired: `.test.tsx` files run with DOM
// globals and React can mount into them. The component tests added in later M9
// phases (registry projections, the Objects section) rely on this path.
describe("jsdom + react mount", () => {
  it("renders a component into a real DOM node", () => {
    const host = document.createElement("div");
    document.body.appendChild(host);
    act(() => {
      createRoot(host).render(<button type="button">Render</button>);
    });
    expect(host.querySelector("button")?.textContent).toBe("Render");
  });
});
