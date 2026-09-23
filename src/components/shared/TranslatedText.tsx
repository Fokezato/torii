import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { translateText } from "@/lib/tools";
import { cn } from "@/lib/utils";

/// Texto que só existe em inglês (sinopse da AniList, notícias da ANN),
/// traduzido pro idioma da interface. Mostra o original, meio apagado,
/// enquanto a tradução chega; se ela falhar, fica o original.
export function TranslatedText({
  text,
  as: Tag = "p",
  className,
}: {
  text: string;
  as?: "p" | "span" | "h3";
  className?: string;
}) {
  const { i18n } = useTranslation();
  const target = i18n.language === "en" ? null : "pt";
  const { data } = useQuery({
    queryKey: ["translate", target, text],
    queryFn: () => translateText(text, target!),
    enabled: target !== null,
    staleTime: Infinity,
    gcTime: 30 * 60_000,
  });
  const pending = target !== null && data === undefined;
  return (
    <Tag className={cn(className, "transition-opacity", pending && "opacity-40")}>
      {target ? (data ?? text) : text}
    </Tag>
  );
}
