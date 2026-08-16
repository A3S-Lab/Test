import { useId, useMemo, useState } from "react";
import {
  componentCatalogSize,
  filterComponentCatalog,
} from "./component-catalog";
import { reviewCategoryLabel, useReviewI18n } from "./review-locale";

export type ComponentCatalogViewProps = {
  selected: string;
  onSelect(value: string): void;
};

export function ComponentCatalogView({
  selected,
  onSelect,
}: ComponentCatalogViewProps) {
  const { t } = useReviewI18n();
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const resultsId = `${useId().replace(/:/g, "")}-component-catalog`;
  const groups = useMemo(() => filterComponentCatalog(query), [query]);
  const resultCount = groups.reduce(
    (count, group) => count + group.components.length,
    0,
  );
  const total = componentCatalogSize();

  return <section className="a3s-catalog" data-component-count={total}>
    <button type="button" className="a3s-disclosure" aria-label={t("componentCatalog", { count: total })} aria-expanded={open} aria-controls={resultsId} onClick={() => setOpen((current) => !current)}>{t("componentCatalog", { count: total })}</button>
    {open && <div id={resultsId} className="a3s-catalog-content">
      <label>{t("searchCatalog")}<input type="search" aria-label={t("searchComponentCatalog")} maxLength={128} value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("searchCatalogPlaceholder")} /></label>
      <small role="status">{t(resultCount === 1 ? "componentTypeOne" : "componentTypeMany", { count: resultCount })}</small>
      <div className="a3s-catalog-results" aria-label={t("componentCatalogResults")}>
      {groups.map((group) => <section key={group.name} aria-label={reviewCategoryLabel(t, group.name)}>
        <strong>{reviewCategoryLabel(t, group.name)}</strong>
        <div>{group.components.map((component) => <button key={component.name} type="button" aria-pressed={component.name === selected} className={component.name === selected ? "selected" : ""} onClick={() => onSelect(component.name)}>{component.name}</button>)}</div>
      </section>)}
      {groups.length === 0 && <p className="a3s-catalog-empty">{t("noCatalogMatches")}</p>}
      </div>
    </div>}
  </section>;
}
