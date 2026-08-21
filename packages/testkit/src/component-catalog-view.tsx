import { useId, useMemo, useState } from "react";
import {
  componentCatalogItemLabel,
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
  const { locale, t } = useReviewI18n();
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
    <button type="button" className="a3s-disclosure" aria-label={t("componentCatalog", { count: total })} aria-expanded={open} aria-controls={resultsId} onClick={() => setOpen((current) => !current)}><span className="a3s-catalog-icon" aria-hidden="true"><CatalogGlyph name="catalog" /></span><span>{t("componentCatalog", { count: total })}</span><i aria-hidden="true" /></button>
    {open && <div id={resultsId} className="a3s-catalog-content">
      <label className="a3s-catalog-search"><span>{t("searchCatalog")}</span><span className="a3s-catalog-search-control"><CatalogGlyph name="search" /><input type="search" aria-label={t("searchComponentCatalog")} maxLength={128} value={query} onChange={(event) => setQuery(event.target.value)} placeholder={t("searchCatalogPlaceholder")} /></span></label>
      <small className="a3s-catalog-count" role="status">{t(resultCount === 1 ? "componentTypeOne" : "componentTypeMany", { count: resultCount })}</small>
      <div className="a3s-catalog-results" aria-label={t("componentCatalogResults")}>
      {groups.map((group) => <section key={group.name} aria-label={reviewCategoryLabel(t, group.name)}>
        <strong>{reviewCategoryLabel(t, group.name)}</strong>
        <div>{group.components.map((component) => {
          const label = componentCatalogItemLabel(component, locale);
          const isSelected = component.name === selected || component.zhCNName === selected;
          return <button key={component.name} type="button" aria-pressed={isSelected} className={isSelected ? "selected" : ""} title={label} onClick={() => onSelect(label)}>{label}</button>;
        })}</div>
      </section>)}
      {groups.length === 0 && <div className="a3s-catalog-empty"><CatalogGlyph name="search" /><p>{t("noCatalogMatches")}</p></div>}
      </div>
    </div>}
  </section>;
}

function CatalogGlyph({ name }: { name: "catalog" | "search" }) {
  if (name === "search") {
    return <svg viewBox="0 0 20 20" aria-hidden="true"><circle cx="8.5" cy="8.5" r="4.75" /><path d="m12 12 4 4" /></svg>;
  }
  return <svg viewBox="0 0 20 20" aria-hidden="true"><rect x="3.5" y="4" width="5" height="5" rx="1" /><rect x="11.5" y="4" width="5" height="5" rx="1" /><rect x="3.5" y="11" width="5" height="5" rx="1" /><rect x="11.5" y="11" width="5" height="5" rx="1" /></svg>;
}
