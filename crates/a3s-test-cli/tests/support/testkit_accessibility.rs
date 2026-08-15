use std::process::Output;

use super::testkit_browser::{
    accessible_ref, assert_wcag_accessibility, click_accessible, encoded_eval, eval,
    select_accessible, wait_for,
};

pub fn exercise_review_candidate_accessibility(command: &impl Fn(&[&str]) -> Output) {
    click_accessible(
        command,
        "seed contract and design review candidates from the audit fixture",
        "button",
        "Seed contract and design candidates",
    );
    wait_for(
        command,
        "wait for review candidates",
        "document.querySelector('#audit-status')?.textContent==='Candidate seeding requested. Both candidates will appear in Review.'&&Boolean(document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-quality'))&&Boolean(document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-design-audit'))",
    );
    assert_wcag_accessibility(command, "audit contract and design review candidates");

    let reseeded = eval(
        command,
        "restore revision-bound design review candidates after the audit",
        "window.testkitFixture.seedReviewCandidates()",
    );
    assert_eq!(reseeded, true, "review candidates were not restored");
    wait_for(
        command,
        "wait for the restored design review candidate",
        "Boolean(document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-design-audit'))",
    );
    let opened_design_editor = eval(
        command,
        "open the revision-bound design suggestion editor",
        "(()=>{const shadow=document.querySelector('[data-a3s-testkit-overlay]').shadowRoot;const button=shadow.querySelector('[aria-label=\"Review design suggestion: The primary action lacks emphasis\"]');if(!button)return false;button.click();return true})()",
    );
    assert_eq!(
        opened_design_editor, true,
        "the design suggestion disappeared before review"
    );
    wait_for(
        command,
        "wait for the design suggestion editor",
        "Boolean(document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-editor'))",
    );
    assert_wcag_accessibility(command, "audit the design suggestion editor");
    click_accessible(
        command,
        "cancel the design suggestion editor",
        "button",
        "Cancel",
    );

    click_accessible(
        command,
        "open the contract finding editor",
        "button",
        "Review contract finding: Use the contracted role",
    );
    wait_for(
        command,
        "wait for the contract finding editor",
        "Boolean(document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-editor'))",
    );
    assert_wcag_accessibility(command, "audit the contract finding editor");
    click_accessible(
        command,
        "cancel the contract finding editor",
        "button",
        "Cancel",
    );

    click_accessible(
        command,
        "dismiss the contract finding",
        "button",
        "Dismiss contract finding: Use the contracted role",
    );
    wait_for(
        command,
        "clear the contract finding",
        "window[Symbol.for('a3s.test.page-context')].listQualityReports().length===0&&window[Symbol.for('a3s.test.page-context')].listDesignAuditReports().length===1",
    );
    click_accessible(
        command,
        "dismiss the design suggestion",
        "button",
        "Dismiss design suggestion: The primary action lacks emphasis",
    );
    wait_for(
        command,
        "clear the design suggestion",
        "window[Symbol.for('a3s.test.page-context')].listQualityReports().length===0&&window[Symbol.for('a3s.test.page-context')].listDesignAuditReports().length===0",
    );
}

pub fn exercise_repair_status_accessibility(command: &impl Fn(&[&str]) -> Output) {
    select_accessible(
        command,
        "select the clarification repair state",
        "combobox",
        "Repair state",
        "needs_input",
    );
    click_accessible(
        command,
        "apply the clarification repair state",
        "button",
        "Apply repair state",
    );
    wait_for(
        command,
        "wait for the repair clarification state",
        "document.querySelector('#audit-status')?.textContent==='Repair state is now needs_input.'&&Boolean(document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('[aria-label^=\"Reply about repair:\"]'))",
    );
    assert_wcag_accessibility(command, "audit the repair clarification state");
    click_accessible(
        command,
        "open the repair reply editor",
        "button",
        "Reply about repair: Repair the broken action",
    );
    wait_for(
        command,
        "wait for the repair reply editor",
        "Boolean(document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-reply-label'))",
    );
    assert_wcag_accessibility(command, "audit the repair reply editor");
    click_accessible(
        command,
        "cancel the repair reply editor",
        "button",
        "Cancel reply",
    );

    select_accessible(
        command,
        "select the human-review repair state",
        "combobox",
        "Repair state",
        "review_ready",
    );
    click_accessible(
        command,
        "apply the human-review repair state",
        "button",
        "Apply repair state",
    );
    wait_for(
        command,
        "wait for the human-review repair state",
        "document.querySelector('#audit-status')?.textContent==='Repair state is now review_ready.'",
    );
    for name in [
        "Accept repair: Repair the broken action",
        "Reject repair: Repair the broken action",
        "Reopen repair: Repair the broken action",
    ] {
        accessible_ref(command, "locate a human review action", "button", name);
    }
    assert_wcag_accessibility(command, "audit the human repair review state");

    for status in ["resolved", "dismissed", "cancelled", "failed"] {
        select_accessible(
            command,
            &format!("select the {status} terminal repair state"),
            "combobox",
            "Repair state",
            status,
        );
        click_accessible(
            command,
            &format!("apply the {status} terminal repair state"),
            "button",
            "Apply repair state",
        );
        wait_for(
            command,
            &format!("wait for the {status} terminal repair state"),
            &format!(
                "document.querySelector('#audit-status')?.textContent==='Repair state is now {status}.'"
            ),
        );
        accessible_ref(
            command,
            &format!("locate the {status} terminal repair action"),
            "button",
            "Reopen repair: Repair the broken action",
        );
        assert_wcag_accessibility(command, &format!("audit the {status} repair state"));
    }
}

pub fn verify_audit_fixture_reset(command: &impl Fn(&[&str]) -> Output) {
    click_accessible(
        command,
        "reset the screen-reader audit fixture",
        "button",
        "Reset fixture",
    );
    wait_for(
        command,
        "wait for the reset screen-reader audit fixture",
        "document.documentElement.dataset.hydrated==='true'&&Boolean(document.querySelector('[data-a3s-testkit-overlay]'))&&window[Symbol.for('a3s.test.page-context')].listRepairs().length===0",
    );
    let reset = encoded_eval(
        command,
        "inspect the reset screen-reader audit fixture",
        "JSON.stringify({url:location.pathname,local:Object.keys(localStorage).filter(key=>key.startsWith('a3s-test.')||key.startsWith('a3s-testkit-')),session:Object.keys(sessionStorage).filter(key=>key.startsWith('a3s-test.')||key.startsWith('a3s-testkit-')),status:document.querySelector('#audit-status')?.textContent})",
    );
    assert_eq!(reset["url"], "/testkit.html");
    assert_eq!(reset["local"], serde_json::json!([]));
    assert_eq!(reset["session"], serde_json::json!([]));
    assert_eq!(reset["status"], "");
    assert_wcag_accessibility(command, "audit the reset screen-reader fixture");
}
