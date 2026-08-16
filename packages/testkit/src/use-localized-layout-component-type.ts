import { useRef, useState } from "react";
import {
  localizeComponentCatalogName,
  type ComponentCatalogLocale,
} from "./component-catalog";
import { useBrowserLayoutEffect } from "./react-effect";

export function useLocalizedLayoutComponentType(
  locale: ComponentCatalogLocale,
) {
  const [componentType, setComponentType] = useState(() => (
    localizeComponentCatalogName("Section", locale)
  ));
  const previousLocale = useRef(locale);

  useBrowserLayoutEffect(() => {
    if (previousLocale.current === locale) return;
    previousLocale.current = locale;
    setComponentType((current) => localizeComponentCatalogName(current, locale));
  }, [locale]);

  return [componentType, setComponentType] as const;
}
