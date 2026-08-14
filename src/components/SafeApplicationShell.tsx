import { useState } from "react";

import type { GraphicsDiagnostic } from "../application/graphics";
import { ActionButton, ApplicationHeader, InlineNotice } from "../ui";

type SafeSurface = "welcome" | "settings" | "diagnostic";
type SafeSettingsSection = "performance" | "photoshop";

interface SafeApplicationShellProps {
  diagnostic: GraphicsDiagnostic;
}

export function SafeApplicationShell({
  diagnostic,
}: SafeApplicationShellProps) {
  const [surface, setSurface] = useState<SafeSurface>("welcome");

  return (
    <main className="safe-application-shell">
      <ApplicationHeader status="modo seguro" />
      <div className="safe-application-shell__body">
        <section className="startup-card safe-shell">
          <nav
            className="safe-shell-navigation"
            aria-label="Superfícies globais"
          >
            <SurfaceButton
              active={surface === "welcome"}
              onPress={() => setSurface("welcome")}
            >
              Boas-vindas
            </SurfaceButton>
            <SurfaceButton
              active={surface === "settings"}
              onPress={() => setSurface("settings")}
            >
              Configurações
            </SurfaceButton>
            <SurfaceButton
              active={surface === "diagnostic"}
              onPress={() => setSurface("diagnostic")}
            >
              Diagnóstico
            </SurfaceButton>
          </nav>
          {surface === "welcome" && (
            <WelcomeSurface
              diagnostic={diagnostic}
              onOpenDiagnostic={() => setSurface("diagnostic")}
              onOpenSettings={() => setSurface("settings")}
            />
          )}
          {surface === "settings" && (
            <SettingsSurface
              onOpenDiagnostic={() => setSurface("diagnostic")}
            />
          )}
          {surface === "diagnostic" && (
            <DiagnosticSurface diagnostic={diagnostic} />
          )}
        </section>
      </div>
    </main>
  );
}

function SurfaceButton({
  active,
  children,
  onPress,
}: {
  active: boolean;
  children: string;
  onPress(): void;
}) {
  return (
    <ActionButton
      aria-pressed={active}
      density="compact"
      onClick={onPress}
      variant={active ? "primary" : "quiet"}
    >
      {children}
    </ActionButton>
  );
}

function WelcomeSurface({
  diagnostic,
  onOpenDiagnostic,
  onOpenSettings,
}: {
  diagnostic: GraphicsDiagnostic;
  onOpenDiagnostic(): void;
  onOpenSettings(): void;
}) {
  return (
    <div className="safe-shell-content">
      <p className="eyebrow">MyAlbuns</p>
      <h1>Boas-vindas</h1>
      <p>
        A criação e a abertura de Projetos permanecem bloqueadas porque este
        computador não confirmou a aceleração gráfica exigida pelo editor.
      </p>
      <p role="status">{diagnostic.reason}</p>
      <div className="safe-shell-actions">
        <ActionButton onClick={onOpenSettings}>
          Abrir Configurações
        </ActionButton>
        <ActionButton onClick={onOpenDiagnostic}>
          Ver diagnóstico
        </ActionButton>
      </div>
      <InlineNotice>
        Somente o editor está bloqueado. Estas superfícies globais continuam
        funcionando sem iniciar uma Sessão do Projeto ou um Canvas.
      </InlineNotice>
    </div>
  );
}

function SettingsSurface({
  onOpenDiagnostic,
}: {
  onOpenDiagnostic(): void;
}) {
  const [section, setSection] =
    useState<SafeSettingsSection>("performance");

  return (
    <div className="safe-shell-content">
      <p className="eyebrow">Preferências globais</p>
      <h1>Configurações do aplicativo</h1>
      <div
        className="safe-settings-tabs"
        role="tablist"
        aria-label="Seções de Configurações"
      >
        <SettingsTab
          active={section === "performance"}
          onPress={() => setSection("performance")}
        >
          Desempenho
        </SettingsTab>
        <SettingsTab
          active={section === "photoshop"}
          onPress={() => setSection("photoshop")}
        >
          Photoshop
        </SettingsTab>
      </div>
      {section === "performance" ? (
        <section
          className="safe-settings-panel"
          role="tabpanel"
          aria-label="Desempenho"
        >
          <h2>Desempenho</h2>
          <p>
            O editor exige WebGL2 com aceleração por hardware. Cache e outras
            preferências globais permanecem fora da Sessão do Projeto.
          </p>
          <ActionButton onClick={onOpenDiagnostic}>
            Ver diagnóstico gráfico
          </ActionButton>
        </section>
      ) : (
        <section
          className="safe-settings-panel"
          role="tabpanel"
          aria-label="Photoshop"
        >
          <h2>Photoshop</h2>
          <p>
            A integração com Photoshop é uma preferência global e não depende
            da inicialização do editor.
          </p>
          <InlineNotice>
            Nenhuma instalação foi consultada por este diagnóstico gráfico.
          </InlineNotice>
        </section>
      )}
      <p className="safe-shell-scope-note">
        Este gate preserva o acesso às superfícies globais. A gestão completa
        de Cache e a detecção do Photoshop continuam nos módulos próprios,
        sem implementações fictícias neste shell.
      </p>
    </div>
  );
}

function SettingsTab({
  active,
  children,
  onPress,
}: {
  active: boolean;
  children: string;
  onPress(): void;
}) {
  return (
    <button
      type="button"
      role="tab"
      aria-selected={active}
      onClick={onPress}
    >
      {children}
    </button>
  );
}

function DiagnosticSurface({
  diagnostic,
}: {
  diagnostic: GraphicsDiagnostic;
}) {
  return (
    <div className="safe-shell-content">
      <p className="eyebrow">Editor bloqueado</p>
      <h1>Diagnóstico gráfico</h1>
      <p>{diagnostic.reason}</p>
      <dl className="diagnostic-list">
        <div>
          <dt>Backend detectado</dt>
          <dd>{diagnostic.renderer}</dd>
        </div>
        <div>
          <dt>Requisito</dt>
          <dd>WebGL2 com aceleração por hardware</dd>
        </div>
        {diagnostic.limits && (
          <>
            <div>
              <dt>Textura máxima informada</dt>
              <dd>{diagnostic.limits.maxTextureSizePx.toLocaleString("pt-BR")} px</dd>
            </div>
            <div>
              <dt>Renderbuffer máximo informado</dt>
              <dd>
                {diagnostic.limits.maxRenderbufferSizePx.toLocaleString(
                  "pt-BR",
                )}{" "}
                px
              </dd>
            </div>
          </>
        )}
      </dl>
      <InlineNotice tone="warning">
        Reative a aceleração por hardware para abrir o editor com desempenho e
        composição visual consistentes.
      </InlineNotice>
    </div>
  );
}
