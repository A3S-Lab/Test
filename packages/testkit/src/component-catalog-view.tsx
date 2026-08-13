import { useId, useMemo, useState } from "react";
import {
  componentCatalogSize,
  filterComponentCatalog,
} from "./component-catalog";

export type ComponentCatalogViewProps = {
  selected: string;
  onSelect(value: string): void;
};

export function ComponentCatalogView({
  selected,
  onSelect,
}: ComponentCatalogViewProps) {
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
    <button type="button" className="a3s-disclosure" aria-label={`Component catalog · ${total}`} aria-expanded={open} aria-controls={resultsId} onClick={() => setOpen((current) => !current)}>Component catalog · {total}</button>
    {open && <div id={resultsId} className="a3s-catalog-content">
      <label>Search catalog<input type="search" aria-label="Search component catalog" maxLength={128} value={query} onChange={(event) => setQuery(event.target.value)} placeholder="Search by type or purpose" /></label>
      <small role="status">{resultCount} component type{resultCount === 1 ? "" : "s"}</small>
      <div className="a3s-catalog-results" aria-label="Component catalog results">
      {groups.map((group) => <section key={group.name} aria-label={group.name}>
        <strong>{group.name}</strong>
        <div>{group.components.map((component) => <button key={component.name} type="button" aria-pressed={component.name === selected} className={component.name === selected ? "selected" : ""} onClick={() => onSelect(component.name)}>{component.name}</button>)}</div>
      </section>)}
      {groups.length === 0 && <p className="a3s-catalog-empty">No catalog matches. Enter any component type in the free-form field above.</p>}
      </div>
    </div>}
  </section>;
}
