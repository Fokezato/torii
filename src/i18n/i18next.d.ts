import "i18next";
import type ptBR from "./pt-BR";

// Chave de tradução inexistente vira erro de TypeScript (e autocomplete).
declare module "i18next" {
  interface CustomTypeOptions {
    defaultNS: "translation";
    resources: { translation: typeof ptBR };
  }
}
