import type { GraphicsDiagnostic } from "../application/graphics";
import "./EditorUnavailableNotice.css";

/**
 * Welcome content when the editor cannot start on this computer: the reason in
 * plain words, what still works, and the details support needs, selectable.
 */
export function EditorUnavailableNotice({ diagnostic }: { diagnostic: GraphicsDiagnostic }) {
  return (
    <section aria-labelledby="editor-unavailable-title" className="editor-unavailable">
      <h1 id="editor-unavailable-title">Não foi possível iniciar o editor neste computador</h1>
      <p>Por isso, não dá para criar nem abrir projetos agora. As configurações continuam disponíveis.</p>
      <section aria-labelledby="editor-unavailable-details" className="editor-unavailable__details">
        <h2 className="ui-section-heading" id="editor-unavailable-details">Detalhes para o suporte</h2>
        <dl className="ui-copyable-text">
          <div><dt>Motivo</dt><dd>{diagnostic.reason}</dd></div>
          <div><dt>Placa de vídeo detectada</dt><dd>{diagnostic.renderer}</dd></div>
          <div><dt>Requisito</dt><dd>WebGL2 com aceleração por hardware</dd></div>
          {diagnostic.limits && <>
            <div><dt>Textura máxima informada</dt><dd>{diagnostic.limits.maxTextureSizePx.toLocaleString("pt-BR")} px</dd></div>
            <div><dt>Renderbuffer máximo informado</dt><dd>{diagnostic.limits.maxRenderbufferSizePx.toLocaleString("pt-BR")} px</dd></div>
          </>}
        </dl>
      </section>
    </section>
  );
}
