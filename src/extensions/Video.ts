import { Node, mergeAttributes } from "@tiptap/core";

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
        parseHTML: (el) => (el as HTMLVideoElement).getAttribute("src"),
      },
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