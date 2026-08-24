import { useEffect, useState } from "react";

/**
 * Mantém a identidade do último baseline semanticamente distinto. Projeções
 * integrais atravessam IPC como objetos novos; a identidade só deve mudar
 * quando os dados que o formulário considera baseline realmente mudarem.
 */
export function useSemanticBaseline<Value>(
  value: Value,
  signature: string,
): Value {
  const [committed, setCommitted] = useState(() => ({ signature, value }));
  const baseline =
    committed.signature === signature ? committed.value : value;

  useEffect(() => {
    setCommitted((current) =>
      current.signature === signature ? current : { signature, value },
    );
  }, [signature, value]);

  return baseline;
}
