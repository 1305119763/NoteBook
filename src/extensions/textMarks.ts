import { Mark, mergeAttributes } from "@tiptap/core";
import { cssColorToHex } from "../utils/palette";

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    textColor: {
      setTextColor: (color: string) => ReturnType;
      unsetTextColor: () => ReturnType;
    };
    fontSize: {
      setFontSize: (size: string) => ReturnType;
      unsetFontSize: () => ReturnType;
    };
    highlight: {
      setHighlight: (color: string) => ReturnType;
      unsetHighlight: () => ReturnType;
    };
  }
}

function readCssColor(el: HTMLElement, prop: "color" | "backgroundColor"): string | null {
  const raw = el.style?.[prop];
  if (!raw) return null;
  return cssColorToHex(raw) || raw;
}

export const TextColor = Mark.create({
  name: "textColor",
  inclusive: true,

  addAttributes() {
    return {
      color: {
        default: null,
        parseHTML: (element) => readCssColor(element as HTMLElement, "color"),
        renderHTML: (attributes) => {
          if (!attributes.color) return {};
          return { style: `color: ${attributes.color}` };
        },
      },
    };
  },

  parseHTML() {
    return [
      {
        tag: "span",
        getAttrs: (element) => {
          const el = element as HTMLElement;
          if (el.getAttribute("data-collab-ip")) return false;
          if (!el.style?.color) return false;
          return {};
        },
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    return ["span", mergeAttributes(HTMLAttributes), 0];
  },

  addCommands() {
    return {
      setTextColor:
        (color: string) =>
        ({ commands }) =>
          commands.setMark(this.name, { color }),
      unsetTextColor:
        () =>
        ({ commands }) =>
          commands.unsetMark(this.name),
    };
  },
});

export const FontSize = Mark.create({
  name: "fontSize",
  inclusive: true,

  addAttributes() {
    return {
      size: {
        default: null,
        parseHTML: (element) => {
          const el = element as HTMLElement;
          if (el.getAttribute("data-collab-ip")) return null;
          return el.style?.fontSize || null;
        },
        renderHTML: (attributes) => {
          if (!attributes.size) return {};
          return { style: `font-size: ${attributes.size}` };
        },
      },
    };
  },

  parseHTML() {
    return [
      {
        tag: "span",
        getAttrs: (element) => {
          const el = element as HTMLElement;
          if (el.getAttribute("data-collab-ip") || !el.style?.fontSize) return false;
          return {};
        },
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    return ["span", mergeAttributes(HTMLAttributes), 0];
  },

  addCommands() {
    return {
      setFontSize:
        (size: string) =>
        ({ commands }) =>
          commands.setMark(this.name, { size }),
      unsetFontSize:
        () =>
        ({ commands }) =>
          commands.unsetMark(this.name),
    };
  },
});

export const HighlightMark = Mark.create({
  name: "highlight",
  inclusive: true,

  addAttributes() {
    return {
      color: {
        default: null,
        parseHTML: (element) => {
          const el = element as HTMLElement;
          return readCssColor(el, "backgroundColor") || el.getAttribute("data-color");
        },
        renderHTML: (attributes) => {
          if (!attributes.color) return {};
          return {
            "data-color": attributes.color,
            style: `background-color: ${attributes.color}`,
          };
        },
      },
    };
  },

  parseHTML() {
    return [
      {
        tag: "mark",
        getAttrs: (element) => {
          const el = element as HTMLElement;
          if (!el.style?.backgroundColor && !el.getAttribute("data-color")) return false;
          return {};
        },
      },
      {
        tag: "span",
        getAttrs: (element) => {
          const el = element as HTMLElement;
          if (el.getAttribute("data-collab-ip")) return false;
          if (!el.style?.backgroundColor) return false;
          return {};
        },
      },
    ];
  },

  renderHTML({ HTMLAttributes }) {
    return ["span", mergeAttributes(HTMLAttributes), 0];
  },

  addCommands() {
    return {
      setHighlight:
        (color: string) =>
        ({ commands }) =>
          commands.setMark(this.name, { color }),
      unsetHighlight:
        () =>
        ({ commands }) =>
          commands.unsetMark(this.name),
    };
  },
});

/** 局域网共同编辑的临时颜色，结束演示时整段去掉，不覆盖用户自己设的文字颜色。 */
export const CollabMark = Mark.create({
  name: "collab",
  inclusive: false,

  addAttributes() {
    return {
      ip: {
        default: null,
        parseHTML: (element) => (element as HTMLElement).getAttribute("data-collab-ip"),
        renderHTML: (attributes) =>
          attributes.ip ? { "data-collab-ip": attributes.ip, title: attributes.ip } : {},
      },
      color: {
        default: null,
        parseHTML: (element) => {
          const el = element as HTMLElement;
          if (!el.getAttribute("data-collab-ip")) return null;
          return el.style?.color || null;
        },
        renderHTML: (attributes) => (attributes.color ? { style: `color: ${attributes.color}` } : {}),
      },
      font: {
        default: null,
        parseHTML: (element) => {
          const el = element as HTMLElement;
          if (!el.getAttribute("data-collab-ip")) return null;
          return el.getAttribute("data-collab-font");
        },
        renderHTML: (attributes) =>
          attributes.font
            ? { "data-collab-font": attributes.font, style: `font-family: ${attributes.font}` }
            : {},
      },
    };
  },

  parseHTML() {
    return [{ tag: "span[data-collab-ip]" }];
  },

  renderHTML({ HTMLAttributes }) {
    return ["span", mergeAttributes({ class: "nb-collab" }, HTMLAttributes), 0];
  },
});
