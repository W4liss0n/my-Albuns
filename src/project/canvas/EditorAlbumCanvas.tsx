import { useEditorView } from "../../state/editorView";
import { AlbumCanvas, type AlbumCanvasProps } from "./AlbumCanvas";

/**
 * The editor's Canvas, following the viewport of the editor view. Only this
 * component subscribes to it, so a wheel step re-renders the Canvas alone
 * instead of the whole editor.
 */
export function EditorAlbumCanvas(props: Omit<AlbumCanvasProps, "viewport">) {
  const viewport = useEditorView((state) => state.viewport);
  return <AlbumCanvas {...props} viewport={viewport} />;
}
