import { describe, it, expect, afterEach, vi } from "vitest";
import { cleanup, render } from "@testing-library/react";
import ColumnMenu from "./ColumnMenu";

afterEach(cleanup);

function open(x, y) {
  // jsdom lays nothing out, so give the menu a size to work with.
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockReturnValue({
    width: 300,
    height: 500,
    top: 0,
    left: 0,
    right: 300,
    bottom: 500,
    x: 0,
    y: 0,
    toJSON() {},
  });
  const { container } = render(
    <ColumnMenu
      columnName="email"
      x={x}
      y={y}
      methods={["token_hmac"]}
      currentMethod={null}
      currentSalt=""
      onPickMethod={() => {}}
      onSaltChange={() => {}}
      onAnonymize={() => {}}
      anonymizing={false}
      onClose={() => {}}
      isLikelyIdentifier={false}
      declaredAsString={false}
      onDeclareAsString={() => {}}
    />,
  );
  return /** @type {HTMLElement} */ (container.querySelector(".column-menu"));
}

describe("ColumnMenu position", () => {
  it("stays where it was opened when it fits", () => {
    window.innerWidth = 1920;
    window.innerHeight = 1080;
    const menu = open(400, 200);
    expect(menu.style.left).toBe("400px");
    expect(menu.style.top).toBe("200px");
  });

  it("moves back inside the window when it would run past an edge", () => {
    window.innerWidth = 1920;
    window.innerHeight = 1080;
    const menu = open(1850, 900);
    expect(menu.style.left).toBe(`${1920 - 300 - 8}px`);
    expect(menu.style.top).toBe(`${1080 - 500 - 8}px`);
  });
});
