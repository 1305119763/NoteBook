import { Node, mergeAttributes } from "@tiptap/core";
import { canonicalMediaSrc, displaySrc } from "../utils/mediaSrc";

declare module "@tiptap/core" {
  interface Commands<ReturnType> {
    video: {
      /**
       * 在编辑器中插入视频节点
       */
      setVideo: (options: { src: string }) => ReturnType;
    };
  }
}

export const Video = Node.create({
  name: "video",

  group: "block",

  selectable: true,
  draggable: true,

  addAttributes() {
    return {
      src: {
        default: null,
        parseHTML: (el) => canonicalMediaSrc((el as HTMLVideoElement).getAttribute("src") || ""),
        renderHTML: (attrs) => (attrs.src ? { src: attrs.src } : {}),
      },
    };
  },

  addNodeView() {
    return ({ node }) => {
      const el = document.createElement("video");
      el.controls = true;
      el.className = "notebook-inline-media";
      el.setAttribute(
        "style",
        "max-width:100%;max-height:400px;border-radius:8px;display:block;margin:8px 0;background:#000;",
      );
      const apply = (next: typeof node) => {
        const shown = displaySrc(String(next.attrs.src || ""));
        if (el.getAttribute("src") !== shown) el.setAttribute("src", shown);
      };
      apply(node);
      return {
        dom: el,
        update(updated: typeof node) {
          if (updated.type !== node.type) return false;
          apply(updated);
          return true;
        },
        ignoreMutation: () => true,
      };
    };
  },

  parseHTML() {
    return [{ tag: "video" }];
  },

  renderHTML({ HTMLAttributes }) {
    return [
      "video",
      mergeAttributes(
        {
          controls: true,
          style:
            "max-width:100%;max-height:400px;border-radius:8px;display:block;margin:8px 0;background:#000;",
        },
        HTMLAttributes,
      ),
    ];
  },

  addCommands() {
    return {
      setVideo:
        (options) =>
        ({ commands }) => {
          return commands.insertContent({
            type: this.name,
            attrs: options,
          });
        },
    };
  },
});