import Image from "@tiptap/extension-image";
import { mergeAttributes } from "@tiptap/core";
import { canonicalMediaSrc, displaySrc } from "../utils/mediaSrc";

const MIN_WIDTH = 48;

export const ResizableImage = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      src: {
        default: null,
        parseHTML: (element) => canonicalMediaSrc(element.getAttribute("src") || ""),
        renderHTML: (attributes) => (attributes.src ? { src: attributes.src } : {}),
      },
      width: {
        default: null,
        parseHTML: (element) => {
          const el = element as HTMLElement;
          return el.getAttribute("width") || el.style?.width || null;
        },
        renderHTML: (attributes) => {
          if (!attributes.width) return {};
          const width = String(attributes.width);
          const css = /px|%$/.test(width) ? width : `${width}px`;
          return { width: css, style: `width: ${css}; height: auto;` };
        },
      },
    };
  },

  renderHTML({ HTMLAttributes }) {
    return ["img", mergeAttributes(this.options.HTMLAttributes, HTMLAttributes)];
  },

  addNodeView() {
    return ({ node, editor, getPos }) => {
      let current = node;
      const wrap = document.createElement("span");
      wrap.className = "nb-resizable-image";
      wrap.contentEditable = "false";

      const img = document.createElement("img");
      img.className = "notebook-inline-media";
      img.draggable = false;
      wrap.appendChild(img);

      const corners = ["nw", "ne", "sw", "se"] as const;
      const handles = corners.map((corner) => {
        const handle = document.createElement("span");
        handle.className = `nb-resize-handle nb-resize-handle--${corner}`;
        handle.dataset.corner = corner;
        handle.title = "拖动调整大小";
        wrap.appendChild(handle);
        return handle;
      });

      const apply = (next: typeof node) => {
        current = next;
        const src = String(next.attrs.src || "");
        const shown = displaySrc(src);
        if (img.getAttribute("src") !== shown) img.setAttribute("src", shown);
        img.alt = String(next.attrs.alt || "");
        const width = next.attrs.width ? String(next.attrs.width) : "";
        img.style.width = width ? (/px|%$/.test(width) ? width : `${width}px`) : "";
        img.style.height = "auto";
        img.style.maxWidth = "100%";
      };
      apply(node);

      const cleanups: Array<() => void> = [];
      const onPointerDown = (event: PointerEvent) => {
        if (!editor.isEditable) return;
        const corner = (event.currentTarget as HTMLElement).dataset.corner || "se";
        event.preventDefault();
        event.stopPropagation();
        const startX = event.clientX;
        const startY = event.clientY;
        const startW = img.getBoundingClientRect().width || MIN_WIDTH;
        const startH = img.getBoundingClientRect().height || MIN_WIDTH;
        const ratio = startW / Math.max(startH, 1);
        const move = (ev: PointerEvent) => {
          let dx = ev.clientX - startX;
          let dy = ev.clientY - startY;
          if (corner === "nw" || corner === "sw") dx = -dx;
          if (corner === "nw" || corner === "ne") dy = -dy;
          const fromW = startW + dx;
          const fromH = (startH + dy) * ratio;
          const next = Math.max(MIN_WIDTH, Math.round((fromW + fromH) / 2));
          img.style.width = `${next}px`;
        };
        const up = () => {
          window.removeEventListener("pointermove", move);
          window.removeEventListener("pointerup", up);
          const pos = getPos();
          if (typeof pos !== "number") return;
          const width = Math.max(MIN_WIDTH, Math.round(img.getBoundingClientRect().width));
          const tr = editor.state.tr.setNodeMarkup(pos, undefined, {
            ...current.attrs,
            width: `${width}px`,
          });
          editor.view.dispatch(tr);
        };
        window.addEventListener("pointermove", move);
        window.addEventListener("pointerup", up);
      };

      for (const handle of handles) {
        handle.addEventListener("pointerdown", onPointerDown);
        cleanups.push(() => handle.removeEventListener("pointerdown", onPointerDown));
      }

      return {
        dom: wrap,
        update(updated: typeof node) {
          if (updated.type !== node.type) return false;
          apply(updated);
          return true;
        },
        selectNode() {
          wrap.classList.add("is-selected");
        },
        deselectNode() {
          wrap.classList.remove("is-selected");
        },
        stopEvent(event: Event) {
          const target = event.target as HTMLElement | null;
          return !!target?.closest?.(".nb-resize-handle");
        },
        ignoreMutation: () => true,
        destroy() {
          cleanups.forEach((fn) => fn());
        },
      };
    };
  },
});
