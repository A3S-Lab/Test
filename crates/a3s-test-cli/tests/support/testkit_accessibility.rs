use std::process::Output;

use super::testkit_browser::{
    accessible_ref, assert_wcag_accessibility, click_accessible, encoded_eval, eval, wait_for,
};

pub fn exercise_review_candidate_accessibility(command: &impl Fn(&[&str]) -> Output) {
    let seeded = eval(
        command,
        "seed contract and design review candidates",
        "window.testkitFixture.seedReviewCandidates()",
    );
    assert_eq!(seeded, true, "review candidates were not accepted");
    wait_for(
        command,
        "wait for review candidates",
        "Boolean(document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-quality'))&&Boolean(document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('.a3s-design-audit'))",
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
        "clear review candidates",
        "window[Symbol.for('a3s.test.page-context')].listQualityReports().length===0&&window[Symbol.for('a3s.test.page-context')].listDesignAuditReports().length===0",
    );
}

pub fn exercise_repair_status_accessibility(command: &impl Fn(&[&str]) -> Output) {
    let needs_input = encoded_eval(
        command,
        "move a repair to needs input",
        "(()=>{const bridge=window[Symbol.for('a3s.test.page-context')];const repair=bridge.listRepairs()[0];const timestamp=new Date().toISOString();bridge.applyRepairEvent({requestId:'a11y-status-1',findingId:repair.id,sequence:1,status:'claimed',actor:'agent',timestamp});bridge.applyRepairEvent({requestId:'a11y-status-2',findingId:repair.id,sequence:2,status:'needs_input',actor:'agent',timestamp,message:'Should the label remain unchanged?'});return JSON.stringify(bridge.listRepairs()[0])})()",
    );
    assert_eq!(needs_input["status"], "needs_input");
    wait_for(
        command,
        "wait for the repair clarification state",
        "Boolean(document.querySelector('[data-a3s-testkit-overlay]').shadowRoot.querySelector('[aria-label^=\"Reply about repair:\"]'))",
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

    let review_ready = encoded_eval(
        command,
        "move a repair to human review",
        "(()=>{const bridge=window[Symbol.for('a3s.test.page-context')];const repair=bridge.listRepairs()[0];const timestamp=new Date().toISOString();for(const [sequence,status,actor] of [[3,'queued','a3s-test'],[4,'claimed','agent'],[5,'repairing','agent'],[6,'verifying','agent'],[7,'review_ready','a3s-test']])bridge.applyRepairEvent({requestId:`a11y-status-${sequence}`,findingId:repair.id,sequence,status,actor,timestamp});return JSON.stringify(bridge.listRepairs()[0])})()",
    );
    assert_eq!(review_ready["status"], "review_ready");
    for name in [
        "Accept repair: Repair the broken action",
        "Reject repair: Repair the broken action",
        "Reopen repair: Repair the broken action",
    ] {
        accessible_ref(command, "locate a human review action", "button", name);
    }
    assert_wcag_accessibility(command, "audit the human repair review state");

    let resolved = encoded_eval(
        command,
        "move a repair to resolved",
        "(()=>{const bridge=window[Symbol.for('a3s.test.page-context')];const repair=bridge.listRepairs()[0];bridge.applyRepairEvent({requestId:'a11y-status-8',findingId:repair.id,sequence:8,status:'resolved',actor:'a3s-test',timestamp:new Date().toISOString()});return JSON.stringify(bridge.listRepairs()[0])})()",
    );
    assert_eq!(resolved["status"], "resolved");
    accessible_ref(
        command,
        "locate the terminal repair action",
        "button",
        "Reopen repair: Repair the broken action",
    );
    assert_wcag_accessibility(command, "audit the terminal repair state");
}
