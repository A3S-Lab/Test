use std::process::Output;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

const RESTORED_DRAFT: &str = "Keep this browser draft across reload";
const EDITED_DRAFT: &str = "Edit this restored draft from its marker";

pub fn run_review_workflow(command: &impl Fn(&[&str]) -> Output) {
    verify_shortcut_discoverability(command);
    exercise_keyboard_multi_selection(command);
    create_and_restore_draft(command);
    edit_draft_from_spatial_marker(command);
    exercise_keyboard_controls(command);
    exercise_host_interaction_blocking(command);
    author_searchable_layout_placement(command);
}

pub fn assert_wcag_accessibility(command: &impl Fn(&[&str]) -> Output, context: &str) {
    let violations = encoded_eval(
        command,
        context,
        "(async()=>JSON.stringify(await window.testkitFixture.auditAccessibility()))()",
    );
    assert_eq!(
        violations,
        serde_json::json!([]),
        "{context} found WCAG violations: {violations:#}"
    );
}

pub fn assert_wcag_accessibility_across_themes(command: &impl Fn(&[&str]) -> Output) {
    for (theme, next_theme) in [("system", "light"), ("light", "dark"), ("dark", "system")] {
        wait_for(
            command,
            &format!("wait for the {theme} review theme"),
            &format!(
                "document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-root')?.dataset.theme==='{}'",
                theme
            ),
        );
        assert_wcag_accessibility(command, &format!("audit the {theme} review theme"));
        click_accessible(
            command,
            &format!("cycle the {theme} review theme"),
            "button",
            &format!("Change overlay theme; current theme is {theme}"),
        );
        wait_for(
            command,
            &format!("wait for the {next_theme} review theme"),
            &format!(
                "document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-root')?.dataset.theme==='{}'",
                next_theme
            ),
        );
    }
}

fn exercise_keyboard_multi_selection(command: &impl Fn(&[&str]) -> Output) {
    run(
        command,
        "focus the first keyboard multi-select target",
        &["focus", "#sticky"],
    );
    click_accessible(
        command,
        "start keyboard multi-selection",
        "button",
        "Mark multi",
    );
    wait_for(
        command,
        "keep keyboard multi-selection in the application",
        "document.activeElement?.id==='sticky'&&!document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-editor')",
    );

    run(
        command,
        "add the first keyboard multi-select target",
        &["press", "Enter"],
    );
    let first = encoded_eval(
        command,
        "inspect the first keyboard multi-selection",
        "(()=>{const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;return JSON.stringify({focus:document.activeElement?.id,editor:Boolean(shadow.querySelector('.a3s-editor')),announcement:shadow.querySelector('.a3s-announcer')?.textContent})})()",
    );
    assert_eq!(first["focus"], "sticky");
    assert_eq!(first["editor"], false);
    assert!(
        first["announcement"]
            .as_str()
            .is_some_and(|value| value.contains("1 selected element")),
        "the first keyboard multi-selection was not announced: {first}"
    );

    run(
        command,
        "focus the second keyboard multi-select target",
        &["focus", "#host-probe"],
    );
    run(
        command,
        "add the second keyboard multi-select target",
        &["press", "Enter"],
    );
    let second = encoded_eval(
        command,
        "inspect the second keyboard multi-selection",
        "(()=>{const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;return JSON.stringify({focus:document.activeElement?.id,editor:Boolean(shadow.querySelector('.a3s-editor')),announcement:shadow.querySelector('.a3s-announcer')?.textContent,hostClicks:window.testkitHostClicks})})()",
    );
    assert_eq!(second["focus"], "host-probe");
    assert_eq!(second["editor"], false);
    assert_eq!(second["hostClicks"], 0);
    assert!(
        second["announcement"]
            .as_str()
            .is_some_and(|value| value.contains("2 selected elements")),
        "the second keyboard multi-selection was not announced: {second}"
    );

    run(
        command,
        "finish keyboard multi-selection",
        &["press", "Shift+Enter"],
    );
    wait_for(
        command,
        "open the completed keyboard multi-select editor",
        "(()=>{const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;return shadow.querySelector('.a3s-editor')?.textContent.includes('2 selected elements')&&shadow.activeElement?.matches('textarea')})()",
    );
    assert_wcag_accessibility(command, "audit the keyboard multi-select editor");
    run(
        command,
        "discard the completed keyboard multi-selection from its editor",
        &["press", "Escape"],
    );
    wait_for(
        command,
        "remove the completed keyboard multi-select editor",
        "(()=>{const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;return !shadow.querySelector('.a3s-editor')&&shadow.activeElement?.classList.contains('a3s-panel')})()",
    );

    run(
        command,
        "focus before cancelling keyboard multi-selection",
        &["focus", "#sticky"],
    );
    click_accessible(
        command,
        "start keyboard multi-selection for cancellation",
        "button",
        "Mark multi",
    );
    wait_for(
        command,
        "restore application focus before keyboard multi-select cancellation",
        "document.activeElement?.id==='sticky'&&!document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-editor')",
    );
    run(
        command,
        "add a keyboard multi-select target before cancellation",
        &["press", "Enter"],
    );
    run(
        command,
        "cancel keyboard multi-selection with Escape",
        &["press", "Escape"],
    );
    wait_for(
        command,
        "clear cancelled keyboard multi-selection",
        "(()=>{const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;return document.activeElement?.id==='sticky'&&!shadow.querySelector('.a3s-editor')&&!shadow.querySelector('.a3s-hint')})()",
    );
}

fn verify_shortcut_discoverability(command: &impl Fn(&[&str]) -> Output) {
    let attributes = encoded_eval(
        command,
        "inspect review shortcut attributes",
        "(()=>{const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;const button=(name)=>[...shadow.querySelectorAll('button')].find(candidate=>candidate.textContent.trim()===name||candidate.getAttribute('aria-label')===name);return JSON.stringify({launcher:shadow.querySelector('.a3s-launch')?.getAttribute('aria-keyshortcuts'),panel:shadow.querySelector('.a3s-panel')?.getAttribute('aria-keyshortcuts'),layout:button('Layout')?.getAttribute('aria-keyshortcuts'),pause:button('Pause page animations')?.getAttribute('aria-keyshortcuts'),markers:button('Hide markers')?.getAttribute('aria-keyshortcuts')})})()",
    );
    assert_eq!(attributes["launcher"], "Control+Shift+F Meta+Shift+F");
    assert_eq!(attributes["panel"], "Escape");
    assert_eq!(attributes["layout"], "L");
    assert_eq!(attributes["pause"], "P");
    assert_eq!(attributes["markers"], "H");

    run(
        command,
        "set a compact viewport for shortcut help",
        &["set", "viewport", "390", "667", "1"],
    );
    activate_accessible_with_enter(
        command,
        "open shortcut help through review preferences",
        "button",
        "Review preferences",
    );
    let snapshot = run(
        command,
        "capture shortcut help accessibility",
        &["snapshot"],
    );
    let snapshot = String::from_utf8_lossy(&snapshot.stdout);
    assert!(
        snapshot.contains("heading \"Keyboard shortcuts\"")
            && snapshot.contains("Toggle review")
            && snapshot.contains("Copy selected drafts")
            && snapshot.contains("Letter shortcuts and panel toggle are ignored while typing")
            && snapshot.contains("Escape still cancels active marking or an open finding editor",),
        "review shortcut help was not exposed through the accessibility tree: {snapshot}"
    );
    assert_wcag_accessibility(command, "audit review preferences and shortcut help");
    let hide = accessible_ref(
        command,
        "locate the final compact review preference",
        "button",
        "Hide until tab restart",
    );
    run(
        command,
        "scroll the final compact review preference into view",
        &["scrollintoview", &hide],
    );
    let compact_layout = encoded_eval(
        command,
        "inspect compact shortcut help layout",
        "(()=>{const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;const settings=shadow.querySelector('.a3s-settings');const panel=shadow.querySelector('.a3s-panel');const hide=[...shadow.querySelectorAll('button')].find(candidate=>candidate.textContent.trim()==='Hide until tab restart');const bounds=hide.getBoundingClientRect();const panelBounds=panel.getBoundingClientRect();return JSON.stringify({settingsHeight:settings.clientHeight,viewportHeight:innerHeight,reachable:bounds.top>=panelBounds.top&&bounds.bottom<=panelBounds.bottom})})()",
    );
    assert!(
        compact_layout["settingsHeight"]
            .as_u64()
            .is_some_and(|height| height <= 401),
        "compact review preferences exceeded their viewport share: {compact_layout}"
    );
    assert_eq!(
        compact_layout["reachable"], true,
        "the final review preference was clipped on a compact viewport: {compact_layout}"
    );
    activate_accessible_with_enter(
        command,
        "close shortcut help after inspection",
        "button",
        "Review preferences",
    );
    wait_for(
        command,
        "wait for compact shortcut help to close",
        "document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('[aria-label=\"Review preferences\"]')?.getAttribute('aria-expanded')==='false'",
    );
    run(
        command,
        "restore the Test Kit browser viewport after shortcut help",
        &["set", "viewport", "1280", "720", "2"],
    );
}

fn create_and_restore_draft(command: &impl Fn(&[&str]) -> Output) {
    click_accessible(
        command,
        "choose element marking through the accessibility tree",
        "button",
        "Mark element",
    );
    run(command, "focus the draft target", &["focus", "#sticky"]);
    run(command, "mark the focused target", &["press", "Enter"]);
    wait_for(
        command,
        "wait for the browser draft editor",
        "Boolean(document.querySelector('[data-a3s-testkit-overlay]')?.shadowRoot?.querySelector('.a3s-editor'))",
    );
    fill_accessible(
        command,
        "fill the browser draft instruction",
        "textbox",
        "Requested fix",
        RESTORED_DRAFT,
    );
    click_accessible(command, "save the browser draft", "button", "Add draft");
    wait_for(
        command,
        "persist the browser draft",
        &format!(
            "[...document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelectorAll('.a3s-item')].some(item=>item.textContent.includes({}))&&Object.keys(localStorage).some(key=>key.startsWith('a3s-test.review-drafts/1/'))",
            quoted(RESTORED_DRAFT),
        ),
    );

    run(command, "reload the Test Kit draft fixture", &["reload"]);
    wait_for(
        command,
        "restore the Test Kit bridge after reload",
        "window[Symbol.for('a3s.test.page-context')]?.probe?.().protocol==='a3s.test.page-context/1'&&Boolean(document.querySelector('[data-a3s-testkit-overlay]')?.shadowRoot?.querySelector('.a3s-panel'))",
    );
    wait_for(
        command,
        "restore the page-local browser draft",
        &format!(
            "document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-list')?.textContent.includes({})",
            quoted(RESTORED_DRAFT),
        ),
    );

    let restored = encoded_eval(
        command,
        "inspect the restored browser draft",
        &format!(
            "(()=>{{const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;const button=shadow.querySelector({});const node=window[Symbol.for('a3s.test.page-context')].snapshot({{detail:'forensic'}}).nodes.find(candidate=>candidate.testId==='repair-target');return JSON.stringify({{markerName:button?.getAttribute('aria-label'),nodeId:node?.id,markerCount:shadow.querySelectorAll('.a3s-marker-action').length}})}})()",
            quoted(&format!("[aria-label='Edit draft marker: {RESTORED_DRAFT}']")),
        ),
    );
    assert_eq!(
        restored["markerCount"], 1,
        "restored marker missing: {restored}"
    );
    assert!(
        restored["nodeId"]
            .as_str()
            .is_some_and(|value| !value.is_empty()),
        "restored draft did not bind to the current semantic target: {restored}"
    );
    assert_eq!(
        restored["markerName"],
        format!("Edit draft marker: {RESTORED_DRAFT}")
    );
    assert_wcag_accessibility(command, "audit a restored draft and spatial marker");
}

fn edit_draft_from_spatial_marker(command: &impl Fn(&[&str]) -> Output) {
    let snapshot = run(
        command,
        "capture restored marker accessibility",
        &["snapshot"],
    );
    let snapshot = String::from_utf8_lossy(&snapshot.stdout);
    assert!(
        snapshot.contains(&format!("button \"Edit draft marker: {RESTORED_DRAFT}\"")),
        "restored marker was not exposed as a named button: {snapshot}"
    );
    click_accessible(
        command,
        "open the restored draft from its spatial marker",
        "button",
        &format!("Edit draft marker: {RESTORED_DRAFT}"),
    );
    fill_accessible(
        command,
        "edit the restored marker draft",
        "textbox",
        "Requested fix",
        EDITED_DRAFT,
    );
    assert_wcag_accessibility(command, "audit the restored draft editor");
    click_accessible(
        command,
        "save the restored marker draft",
        "button",
        "Save changes",
    );
    wait_for(
        command,
        "show the edited spatial draft",
        &format!(
            "document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-list')?.textContent.includes({})",
            quoted(EDITED_DRAFT),
        ),
    );
}

fn exercise_keyboard_controls(command: &impl Fn(&[&str]) -> Output) {
    let draft_shortcuts = encoded_eval(
        command,
        "inspect draft shortcut attributes",
        "(()=>{const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;const button=(name)=>[...shadow.querySelectorAll('button')].find(candidate=>candidate.textContent.trim()===name);return JSON.stringify({copy:button('Copy Markdown')?.getAttribute('aria-keyshortcuts'),clear:button('Clear drafts')?.getAttribute('aria-keyshortcuts')})})()",
    );
    assert_eq!(draft_shortcuts["copy"], "C");
    assert_eq!(draft_shortcuts["clear"], "X");
    run(
        command,
        "close the review panel with its global shortcut",
        &["press", "Control+Shift+f"],
    );
    wait_for(
        command,
        "wait for the keyboard-closed panel",
        "!document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-panel')",
    );
    run(
        command,
        "open the review panel with its global shortcut",
        &["press", "Control+Shift+f"],
    );
    wait_for(
        command,
        "wait for the keyboard-opened panel",
        "Boolean(document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-panel'))",
    );
    wait_for(
        command,
        "wait for the fixture motion ownership baseline",
        "(()=>{const state=(id)=>document.getAnimations().find(animation=>animation.effect?.target?.id===id)?.playState;return state('running-motion')==='running'&&state('paused-motion')==='paused'})()",
    );
    run(
        command,
        "pause page motion from the keyboard",
        &["press", "p"],
    );
    wait_for(
        command,
        "wait for the keyboard pause state",
        "(()=>{const state=(id)=>document.getAnimations().find(animation=>animation.effect?.target?.id===id)?.playState;return document.documentElement.hasAttribute('data-a3s-testkit-animations-paused')&&state('running-motion')==='paused'&&state('paused-motion')==='paused'})()",
    );
    encoded_eval(
        command,
        "start new page motion while review motion is paused",
        "(()=>{const element=document.createElement('span');element.id='late-motion';document.querySelector('#motion-probe').append(element);return JSON.stringify(true)})()",
    );
    wait_for(
        command,
        "wait for newly started page motion to pause",
        "document.getAnimations().find(animation=>animation.effect?.target?.id==='late-motion')?.playState==='paused'",
    );
    run(
        command,
        "resume page motion from the keyboard",
        &["press", "p"],
    );
    wait_for(
        command,
        "wait for the keyboard resume state",
        "(()=>{const state=(id)=>document.getAnimations().find(animation=>animation.effect?.target?.id===id)?.playState;return !document.documentElement.hasAttribute('data-a3s-testkit-animations-paused')&&state('running-motion')==='running'&&state('late-motion')==='running'&&state('paused-motion')==='paused'})()",
    );
    run(command, "hide markers from the keyboard", &["press", "h"]);
    wait_for(
        command,
        "wait for keyboard-hidden markers",
        "document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelectorAll('.a3s-marker-action').length===0",
    );
    run(command, "show markers from the keyboard", &["press", "h"]);
    wait_for(
        command,
        "wait for keyboard-shown markers",
        "document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelectorAll('.a3s-marker-action').length===1",
    );
    run(command, "copy drafts from the keyboard", &["press", "c"]);
    wait_for(
        command,
        "wait for the host clipboard adapter",
        &format!(
            "window.testkitCopiedText?.includes({})",
            quoted(EDITED_DRAFT)
        ),
    );
    run(
        command,
        "open Layout Mode from the keyboard",
        &["press", "l"],
    );
    wait_for(
        command,
        "wait for keyboard-opened Layout Mode",
        "Boolean(document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-layout'))",
    );
    run(
        command,
        "close the review panel with Escape",
        &["press", "Escape"],
    );
    wait_for(
        command,
        "wait for the Escape-closed panel",
        "!document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-panel')",
    );
    run(
        command,
        "reopen the review panel after Escape",
        &["press", "Control+Shift+f"],
    );
    wait_for(
        command,
        "wait for the reopened review panel",
        "Boolean(document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-panel'))",
    );
    run(command, "clear drafts from the keyboard", &["press", "x"]);
    wait_for(
        command,
        "wait for keyboard-cleared drafts",
        "document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelectorAll('.a3s-item:not(.submitted)').length===0&&!Object.keys(localStorage).some(key=>key.startsWith('a3s-test.review-drafts/1/'))",
    );
}

fn exercise_host_interaction_blocking(command: &impl Fn(&[&str]) -> Output) {
    click_host_probe(command, "click the unblocked host probe");
    wait_for(
        command,
        "observe the unblocked host click",
        "window.testkitHostClicks===1",
    );
    activate_accessible_with_enter(
        command,
        "open review preferences",
        "button",
        "Review preferences",
    );
    wait_for(
        command,
        "wait for review preferences to open",
        "document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('[aria-label=\"Review preferences\"]')?.getAttribute('aria-expanded')==='true'",
    );
    set_checkbox_accessible(
        command,
        "enable host pointer blocking",
        "Block page pointer input",
        true,
    );
    wait_for(
        command,
        "wait for host pointer blocking",
        "document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('[aria-label=\"Block page pointer input\"]')?.checked===true",
    );
    assert_wcag_accessibility(command, "audit interaction-blocking preferences");
    click_host_probe(command, "attempt the blocked host click");
    let blocked_count = eval(
        command,
        "inspect the blocked host click",
        "window.testkitHostClicks",
    );
    assert_eq!(blocked_count, 1, "blocked host click reached the page");
    set_checkbox_accessible(
        command,
        "disable host pointer blocking from the overlay",
        "Block page pointer input",
        false,
    );
    wait_for(
        command,
        "wait for host pointer unblocking",
        "document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('[aria-label=\"Block page pointer input\"]')?.checked===false",
    );
    click_host_probe(command, "click the unblocked host probe again");
    wait_for(
        command,
        "observe the second unblocked host click",
        "window.testkitHostClicks===2",
    );
    activate_accessible_with_enter(
        command,
        "close review preferences after interaction blocking",
        "button",
        "Review preferences",
    );
    wait_for(
        command,
        "wait for closed review preferences",
        "document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('[aria-label=\"Review preferences\"]')?.getAttribute('aria-expanded')==='false'",
    );
}

fn author_searchable_layout_placement(command: &impl Fn(&[&str]) -> Output) {
    click_accessible(
        command,
        "open the component catalog",
        "button",
        "Component catalog · 90",
    );
    fill_accessible(
        command,
        "search the component catalog",
        "searchbox",
        "Search component catalog",
        "checkout",
    );
    wait_for(
        command,
        "filter the component catalog",
        "(()=>{const results=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-catalog-results');return results?.textContent.includes('Checkout Form')&&!results.textContent.includes('Breadcrumbs')})()",
    );
    assert_wcag_accessibility(command, "audit the filtered Layout component catalog");
    scroll_and_click_accessible(
        command,
        "choose the searched component type",
        "button",
        "Checkout Form",
    );
    let selected_component = eval(
        command,
        "inspect the selected component type",
        "document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('[aria-label=\"Layout component type\"]')?.value",
    );
    assert_eq!(selected_component, "Checkout Form");
    click_accessible(
        command,
        "close the searched component catalog",
        "button",
        "Component catalog · 90",
    );
    wait_for(
        command,
        "wait for the closed component catalog",
        "document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('[aria-label=\"Component catalog · 90\"]')?.getAttribute('aria-expanded')==='false'",
    );
    select_accessible(
        command,
        "choose the wireframe Layout canvas",
        "combobox",
        "Layout canvas",
        "wireframe",
    );
    click_accessible(
        command,
        "start the searched Layout placement",
        "button",
        "Draw placement",
    );
    let marking = encoded_eval(
        command,
        "inspect the searched Layout marking mode",
        "(async()=>{await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;const button=[...shadow.querySelectorAll('button')].find(candidate=>candidate.textContent.trim()==='Draw placement');return JSON.stringify({hint:shadow.querySelector('.a3s-hint')?.textContent??null,pressed:button?.getAttribute('aria-pressed')??null})})()",
    );
    assert!(
        marking["hint"]
            .as_str()
            .is_some_and(|value| value.contains("Drag the intended component region")),
        "Layout marking hint was missing: {marking}"
    );
    assert_eq!(marking["pressed"], "true", "Layout marking state missing");
    run(
        command,
        "move to the Layout placement start",
        &["mouse", "move", "80", "300"],
    );
    run(
        command,
        "press the Layout placement pointer",
        &["mouse", "down", "left"],
    );
    let pointer_down = overlay_state(command, "inspect the Layout pointer down");
    assert_eq!(
        pointer_down["highlight"], true,
        "Layout pointer down did not start a region: {pointer_down}"
    );
    run(
        command,
        "drag the Layout placement pointer",
        &["mouse", "move", "520", "520"],
    );
    let pointer_move = overlay_state(command, "inspect the Layout pointer move");
    assert_eq!(
        pointer_move["highlight"], true,
        "Layout pointer move lost its region: {pointer_move}"
    );
    run(
        command,
        "release the Layout placement pointer",
        &["mouse", "up", "left"],
    );
    let pointer_up = overlay_state(command, "inspect the Layout pointer up");
    assert!(
        pointer_up["editor"]
            .as_str()
            .is_some_and(|value| value.contains("Place Checkout Form")),
        "Layout pointer up did not create an editor: {pointer_up}"
    );
    assert_wcag_accessibility(command, "audit the Layout placement editor");
    click_accessible(
        command,
        "save the searched Layout draft",
        "button",
        "Add draft",
    );
    wait_for(
        command,
        "persist the searched Layout draft",
        "Object.keys(localStorage).some(key=>key.startsWith('a3s-test.review-drafts/1/'))",
    );
    let layout = encoded_eval(
        command,
        "inspect the searched Layout draft",
        "(()=>{const key=Object.keys(localStorage).find(candidate=>candidate.startsWith('a3s-test.review-drafts/1/'));const item=JSON.parse(localStorage.getItem(key)).items[0].draft;return JSON.stringify({instruction:item.instruction,target:item.target})})()",
    );
    assert_eq!(layout["target"]["layout"]["kind"], "placement");
    assert_eq!(layout["target"]["layout"]["componentType"], "Checkout Form");
    assert_eq!(layout["target"]["layout"]["canvas"], "wireframe");
    assert_eq!(layout["target"]["region"]["x"], 80.0);
    assert_eq!(layout["target"]["region"]["y"], 300.0);
    assert_eq!(layout["target"]["region"]["width"], 440.0);
    assert_eq!(layout["target"]["region"]["height"], 220.0);
    run(command, "clear the searched Layout draft", &["press", "x"]);
    wait_for(
        command,
        "wait for the cleared Layout draft",
        "!Object.keys(localStorage).some(key=>key.startsWith('a3s-test.review-drafts/1/'))",
    );
    run(
        command,
        "close Layout Mode from the keyboard",
        &["press", "l"],
    );
}

pub fn verify_hide_until_restart_focus(command: &impl Fn(&[&str]) -> Output) {
    run(
        command,
        "focus the host application before hiding review",
        &["focus", "#host-probe"],
    );
    activate_accessible_with_enter(
        command,
        "open review preferences before hiding review",
        "button",
        "Review preferences",
    );
    wait_for(
        command,
        "wait for review preferences before hiding review",
        "document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('[aria-label=\"Review preferences\"]')?.getAttribute('aria-expanded')==='true'",
    );
    activate_accessible_with_enter(
        command,
        "hide review until the tab restarts",
        "button",
        "Hide until tab restart",
    );
    wait_for(
        command,
        "wait for the hidden review overlay",
        "!document.querySelector('[data-a3s-testkit-overlay]')",
    );
    let focused = eval(
        command,
        "inspect focus after hiding review",
        "document.activeElement?.id",
    );
    assert_eq!(
        focused, "host-probe",
        "hiding review did not restore application focus"
    );
}

fn click_host_probe(command: &impl Fn(&[&str]) -> Output, context: &str) {
    click_accessible(command, context, "button", "Host interaction probe");
}

pub(super) fn click_accessible(
    command: &impl Fn(&[&str]) -> Output,
    context: &str,
    role: &str,
    name: &str,
) {
    let reference = accessible_ref(command, context, role, name);
    run(command, context, &["click", &reference]);
}

fn activate_accessible_with_enter(
    command: &impl Fn(&[&str]) -> Output,
    context: &str,
    role: &str,
    name: &str,
) {
    let reference = accessible_ref(command, context, role, name);
    run(
        command,
        &format!("focus before {context}"),
        &["focus", &reference],
    );
    run(command, context, &["press", "Enter"]);
}

fn scroll_and_click_accessible(
    command: &impl Fn(&[&str]) -> Output,
    context: &str,
    role: &str,
    name: &str,
) {
    let reference = accessible_ref(command, context, role, name);
    run(
        command,
        &format!("scroll before {context}"),
        &["scrollintoview", &reference],
    );
    let reference = accessible_ref(command, context, role, name);
    run(command, context, &["click", &reference]);
}

fn fill_accessible(
    command: &impl Fn(&[&str]) -> Output,
    context: &str,
    role: &str,
    name: &str,
    value: &str,
) {
    let reference = accessible_ref(command, context, role, name);
    run(command, context, &["fill", &reference, value]);
}

pub(super) fn select_accessible(
    command: &impl Fn(&[&str]) -> Output,
    context: &str,
    role: &str,
    name: &str,
    value: &str,
) {
    let reference = accessible_ref(command, context, role, name);
    run(command, context, &["select", &reference, value]);
}

fn set_checkbox_accessible(
    command: &impl Fn(&[&str]) -> Output,
    context: &str,
    name: &str,
    checked: bool,
) {
    let reference = accessible_ref(command, context, "checkbox", name);
    run(
        command,
        &format!("scroll before {context}"),
        &["scrollintoview", &reference],
    );
    let reference = accessible_ref(command, context, "checkbox", name);
    run(
        command,
        context,
        &[if checked { "check" } else { "uncheck" }, &reference],
    );
}

pub(super) fn accessible_ref(
    command: &impl Fn(&[&str]) -> Output,
    context: &str,
    role: &str,
    name: &str,
) -> String {
    let output = run(
        command,
        &format!("snapshot before {context}"),
        &["snapshot"],
    );
    let snapshot = String::from_utf8_lossy(&output.stdout);
    let prefix = format!("- {role} \"{name}\"");
    let matches = snapshot
        .lines()
        .map(str::trim_start)
        .filter(|line| line.starts_with(&prefix))
        .filter_map(snapshot_ref)
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "{context} required one accessible {role} named {name:?}, found {}:\n{snapshot}",
        matches.len(),
    );
    format!("@{}", matches[0])
}

fn snapshot_ref(line: &str) -> Option<&str> {
    let start = line.find("ref=")? + "ref=".len();
    let end = line[start..].find([',', ']'])? + start;
    Some(&line[start..end])
}

pub(super) fn wait_for(command: &impl Fn(&[&str]) -> Output, context: &str, condition: &str) {
    const TIMEOUT: Duration = Duration::from_secs(25);
    const POLL_INTERVAL: Duration = Duration::from_millis(100);

    let expression = format!("Boolean({condition})");
    let deadline = Instant::now() + TIMEOUT;
    let mut attempts = 0_u32;

    loop {
        attempts += 1;
        let output = command(&["eval", &expression]);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if output.status.success() {
            let satisfied: bool = serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
                panic!(
                    "{context} returned an invalid condition result: {error}\nstdout:\n{stdout}\nstderr:\n{stderr}"
                )
            });
            if satisfied {
                return;
            }
        } else if !is_transient_browser_read_failure(&stderr) {
            panic!("{context} failed\nstdout:\n{stdout}\nstderr:\n{stderr}");
        }

        if Instant::now() >= deadline {
            panic!(
                "{context} timed out after {attempts} condition polls\nstdout:\n{stdout}\nstderr:\n{stderr}"
            );
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn is_transient_browser_read_failure(stderr: &str) -> bool {
    stderr.contains("Resource temporarily unavailable")
        || stderr.contains("daemon may be busy or unresponsive")
}

pub(super) fn eval(command: &impl Fn(&[&str]) -> Output, context: &str, script: &str) -> Value {
    let output = run(command, context, &["eval", script]);
    serde_json::from_slice(&output.stdout)
        .unwrap_or_else(|error| panic!("{context} did not return JSON: {error}"))
}

pub(super) fn encoded_eval(
    command: &impl Fn(&[&str]) -> Output,
    context: &str,
    script: &str,
) -> Value {
    let encoded = eval(command, context, script)
        .as_str()
        .unwrap_or_else(|| panic!("{context} did not return an encoded JSON string"))
        .to_string();
    serde_json::from_str(&encoded)
        .unwrap_or_else(|error| panic!("{context} returned invalid encoded JSON: {error}"))
}

fn overlay_state(command: &impl Fn(&[&str]) -> Output, context: &str) -> Value {
    encoded_eval(
        command,
        context,
        "(async()=>{await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;return JSON.stringify({highlight:Boolean(shadow.querySelector('.a3s-highlight')),hint:shadow.querySelector('.a3s-hint')?.textContent??null,editor:shadow.querySelector('.a3s-editor textarea')?.value??null})})()",
    )
}

fn run(command: &impl Fn(&[&str]) -> Output, context: &str, arguments: &[&str]) -> Output {
    let output = command(arguments);
    assert!(
        output.status.success(),
        "{context} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    output
}

fn quoted(value: &str) -> String {
    serde_json::to_string(value).expect("encode bounded browser fixture string")
}
