import { useState } from "react";

import type {
  NewProjectPreset,
  ProjectLaunchFailure,
  ProjectLaunchOutcome,
} from "./application/globalProjectPort";
import "./NewProjectFlow.css";

const NEUTRAL_PRESET: NewProjectPreset = "neutralV1";

interface NewProjectFlowProps {
  onCancel(): void;
  onCreate(preset: NewProjectPreset): Promise<ProjectLaunchOutcome>;
}

type CreationStep = "dimensions" | "personalization";

export function NewProjectFlow({ onCancel, onCreate }: NewProjectFlowProps) {
  const [step, setStep] = useState<CreationStep>("dimensions");
  const [isCreating, setIsCreating] = useState(false);
  const [failure, setFailure] = useState<ProjectLaunchFailure | null>(null);

  const createProject = async () => {
    setIsCreating(true);
    setFailure(null);
    const outcome = await onCreate(NEUTRAL_PRESET);
    if (outcome.status === "failed") {
      setFailure(outcome.error);
    }
    setIsCreating(false);
  };

  return (
    <div className="new-project-backdrop">
      <section
        aria-labelledby="new-project-title"
        aria-modal="true"
        className="new-project-flow"
        role="dialog"
      >
        <header className="new-project-header">
          <div>
            <p className="global-eyebrow">Novo Projeto</p>
            <h1 id="new-project-title">
              {step === "dimensions" ? "Dimensões" : "Personalização"}
            </h1>
          </div>
          <ol aria-label="Etapas da criação" className="new-project-steps">
            <li aria-current={step === "dimensions" ? "step" : undefined}>
              Dimensões
            </li>
            <li
              aria-current={
                step === "personalization" ? "step" : undefined
              }
            >
              Personalização
            </li>
          </ol>
        </header>

        {step === "dimensions" ? (
          <DimensionsStep />
        ) : (
          <PersonalizationStep failure={failure} />
        )}

        <footer className="new-project-footer">
          <div>
            {step === "personalization" ? (
              <button
                disabled={isCreating}
                onClick={() => setStep("dimensions")}
                type="button"
              >
                Voltar
              </button>
            ) : null}
          </div>
          <div className="new-project-footer-actions">
            <button disabled={isCreating} onClick={onCancel} type="button">
              Cancelar
            </button>
            {step === "dimensions" ? (
              <button
                className="new-project-primary-action"
                onClick={() => setStep("personalization")}
                type="button"
              >
                Próximo
              </button>
            ) : (
              <button
                className="new-project-primary-action"
                disabled={isCreating}
                onClick={() => void createProject()}
                type="button"
              >
                {isCreating ? "Criando Projeto…" : "Criar"}
              </button>
            )}
          </div>
        </footer>
      </section>
    </div>
  );
}

function DimensionsStep() {
  return (
    <div className="new-project-content new-project-dimensions">
      <ValueGroup title="Documento">
        <Value label="Unidade" value="mm" />
        <Value label="Lâmina" value="600 × 300 mm (60 × 30 cm)" />
        <Value label="Resolução" value="300 DPI" />
      </ValueGroup>
      <ValueGroup title="Estrutura">
        <Value label="Quantidade" value="2 Lâminas duplas" />
        <Value label="Primeira Lâmina" value="Dupla" />
        <Value label="Última Lâmina" value="Dupla" />
      </ValueGroup>
      <ValueGroup title="Áreas técnicas">
        <Value label="Sangria" value="3 mm" />
        <Value label="Área de segurança" value="3 mm" />
      </ValueGroup>
      <p className="new-project-summary">
        Lâmina 60 × 30 cm · Páginas 30 × 30 cm · 300 DPI
      </p>
    </div>
  );
}

function PersonalizationStep({
  failure,
}: {
  failure: ProjectLaunchFailure | null;
}) {
  return (
    <div className="new-project-content new-project-personalization">
      <div aria-label="Reprodução da Lâmina" className="new-project-preview">
        <div className="new-project-preview-sheet">
          <span />
          <span />
        </div>
      </div>
      <div className="new-project-visual-values">
        <ValueGroup title="Padrões visuais">
          <Value label="Background" value="Background branco" />
          <Value label="Overlay" value="Sem Overlay" />
          <Value label="Padrão dos Frames" value="Sem borda" />
          <Value label="Composição inicial" value="Sem Frames ou mídias" />
        </ValueGroup>
        <p className="new-project-native-note">
          Nome e Localização serão escolhidos no diálogo do Windows ao criar.
        </p>
        {failure ? (
          <section className="global-open-error" role="alert">
            <h2>Não foi possível criar o Projeto</h2>
            <p>{failure.message}</p>
            {failure.action ? <p>{failure.action}</p> : null}
          </section>
        ) : null}
      </div>
    </div>
  );
}

function ValueGroup({
  children,
  title,
}: {
  children: React.ReactNode;
  title: string;
}) {
  return (
    <section className="new-project-value-group">
      <h2>{title}</h2>
      <dl>{children}</dl>
    </section>
  );
}

function Value({ label, value }: { label: string; value: string }) {
  return (
    <div>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}
