# ACL Suite Specification

An A3S Test manifest is an ACL document with exactly one labeled `suite`
block. Unknown blocks and attributes are rejected.

## Suite

```acl
suite "name" {
    version = 1
    scenario "id" {}
}
```

`version` is a positive integer and currently defaults to `1`.

## Scenario

```acl
scenario "stable-id" {
    name = "Human-readable name"
    surface = "web"
    timeout_ms = 30000
}
```

`surface` is required and is one of `web`, `gui`, or `tui`. `timeout_ms`
defaults to 60 seconds and covers surface opening and all scenario steps.

## Actions

Action blocks execute in source order. Each action requires one unique label
inside its scenario.

```acl
navigate "open" {
    url = "https://example.com"
}

snapshot "controls" {
    interactive = true
}

click "submit" {
    target = role("button", "Submit")
}

fill "email" {
    target = label("Email")
    value = "user@example.test"
}

press "confirm" {
    key = "Enter"
}

wait "loaded" {
    load = "networkidle"
}

expect "saved" {
    text = "Saved"
}

screenshot "final" {
    path = "final.png"
}
```

`wait` accepts exactly one of:

- `load = "networkidle"` or `load = "domcontentloaded"`
- `text = "..."`
- `url = "..."`

`expect` accepts exactly one of:

- `text = "..."`
- `url = "..."`
- `visible = <target>`

## Typed targets

```acl
target = ref("@e4")
target = css("[data-testid=save]")
target = role("button", "Save")
target = text("Save")
target = text("Save", true)
target = testid("save")
target = label("Document title")
target = placeholder("Search")
```

The optional second argument to `text` controls exact matching. Visibility
assertions currently require `ref()` or `css()` because those map directly to
the browser visibility protocol.

## Admission behavior

Validation completes before a driver is opened. Errors expose a stable code and
logical path such as:

```text
test.spec.attribute_unknown
suite.office.scenario.word.navigate.open.retries
```

Source values are not needed to route an error. Coding agents should use the
code and path to repair manifests.
