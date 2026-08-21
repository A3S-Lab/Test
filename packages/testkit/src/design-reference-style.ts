export const DESIGN_REFERENCE_CSS = `
.a3s-design-reference {
  display: grid;
  min-height: 66px;
  padding: 10px;
  border: 1px solid var(--a3s-line);
  border-radius: 10px;
  background: var(--a3s-soft);
  grid-template-columns: 36px minmax(0, 1fr) auto;
  align-items: center;
  gap: 9px;
}

.a3s-design-reference.has-reference {
  grid-template-columns: 64px minmax(0, 1fr) auto;
}

.a3s-design-reference > div {
  display: flex;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
}

.a3s-design-reference strong {
  color: var(--a3s-text);
  font-size: 11px;
}

.a3s-design-reference small {
  color: var(--a3s-faint);
  font-size: 9.5px;
}

.a3s-design-reference img {
  width: 64px;
  height: 44px;
  border: 1px solid var(--a3s-line-strong);
  border-radius: 7px;
  background: #ffffff;
  object-fit: cover;
}

.a3s-design-reference-icon {
  display: grid;
  width: 36px;
  height: 36px;
  border-radius: 9px;
  background: var(--a3s-blue-soft);
  color: var(--a3s-blue-ink);
  place-items: center;
}

.a3s-design-reference-icon svg,
.a3s-design-reference-action svg,
.a3s-design-reference-open svg {
  width: 17px;
  height: 17px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1.65;
  stroke-linecap: round;
  stroke-linejoin: round;
}

.a3s-design-reference-action,
.a3s-design-reference-open {
  display: inline-flex;
  white-space: nowrap;
  align-items: center;
  gap: 5px;
}

.a3s-design-reference-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 2px;
}

.a3s-design-reference-action {
  min-height: 30px;
  padding: 0 7px;
  font-size: 10px;
}

.a3s-design-reference-open {
  min-height: 34px;
}

@media (max-width: 420px) {
  .a3s-design-reference:not(.has-reference) {
    grid-template-columns: 36px minmax(0, 1fr);
  }

  .a3s-design-reference:not(.has-reference) .a3s-design-reference-open {
    width: 100%;
    min-height: 44px;
    grid-column: 1 / -1;
    justify-content: center;
  }

  .a3s-design-reference.has-reference {
    grid-template-columns: 56px minmax(0, 1fr);
  }

  .a3s-design-reference.has-reference img {
    width: 56px;
    height: 40px;
  }

  .a3s-design-reference-actions {
    grid-column: 2;
    justify-content: flex-start;
  }

  .a3s-design-reference-action {
    min-height: 40px;
  }
}
`;
