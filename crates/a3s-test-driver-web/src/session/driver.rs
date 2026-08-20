use async_trait::async_trait;

use super::*;

const PAGE_CONTEXT_INSPECT_FUNCTION: &str = r#"(async ({ request, waitTimeoutMs }) => {
  const bridge = window[Symbol.for("a3s.test.page-context")];
  if (!bridge || typeof bridge.probe !== "function" || typeof bridge.snapshot !== "function") {
    return { present: false };
  }
  const probe = bridge.probe();
  if (probe?.protocol !== "a3s.test.page-context/1") return { present: false };
  const capture = () => ({ present: true, ...bridge.snapshot(request) });
  if (request.detail !== "diff" || request.sinceRevision == null || waitTimeoutMs <= 0) {
    return capture();
  }
  if (typeof bridge.waitForDiff === "function") {
    const diff = await bridge.waitForDiff({ ...request, timeoutMs: waitTimeoutMs });
    return diff === null ? capture() : { present: true, ...diff };
  }
  if (typeof bridge.waitForChange === "function") {
    await bridge.waitForChange(request.sinceRevision, waitTimeoutMs);
  }
  return capture();
})"#;

#[async_trait]
impl DriverSession for AgentBrowserSession {
    async fn observe(&mut self) -> Result<SurfaceObservation, DriverError> {
        self.capture_observation(false).await
    }

    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError> {
        self.ensure_open()?;

        match &step.action {
            Action::Navigate { url } => self
                .execute_command(vec!["open".into(), url.into()])
                .await
                .map(|data| StepOutput::new("page opened").with_data(data)),
            Action::Snapshot { interactive } => {
                self.capture_observation(*interactive)
                    .await
                    .map(|observation| StepOutput {
                        summary: "page snapshot captured".to_string(),
                        data: observation.data,
                        evidence: observation.evidence,
                        page_context: observation.page_context,
                    })
            }
            Action::Click { target } => {
                self.execute_target_action(target, "click", None)
                    .await
                    .map(|data| StepOutput::new("target clicked").with_data(data))
            }
            Action::Hover { target } => {
                let args = target_action(target, "hover", None)?;
                self.execute_command(args)
                    .await
                    .map(|data| StepOutput::new("target hovered").with_data(data))
            }
            Action::Focus { target } => {
                let selector = direct_selector(target)?;
                self.execute_command(vec!["focus".into(), selector.into()])
                    .await
                    .map(|data| StepOutput::new("target focused").with_data(data))
            }
            Action::DoubleClick { target } => {
                let selector = direct_selector(target)?;
                self.execute_command(vec!["dblclick".into(), selector.into()])
                    .await
                    .map(|data| StepOutput::new("target double-clicked").with_data(data))
            }
            Action::ContextClick { target } => self.context_click(target).await,
            Action::Fill { target, value } => {
                self.execute_target_action(target, "fill", Some(value))
                    .await
                    .map(|data| StepOutput::new("target filled").with_data(data))
            }
            Action::Type { target, value } => {
                let selector = direct_selector(target)?;
                self.execute_command(vec!["type".into(), selector.into(), value.into()])
                    .await
                    .map(|data| StepOutput::new("text typed into target").with_data(data))
            }
            Action::InsertText { value } => self
                .execute_command(vec!["keyboard".into(), "inserttext".into(), value.into()])
                .await
                .map(|data| StepOutput::new("text inserted at current focus").with_data(data)),
            Action::Check { target } => {
                self.execute_target_action(target, "check", None)
                    .await
                    .map(|data| StepOutput::new("target checked").with_data(data))
            }
            Action::Uncheck { target } => {
                let selector = direct_selector(target)?;
                self.execute_command(vec!["uncheck".into(), selector.into()])
                    .await
                    .map(|data| StepOutput::new("target unchecked").with_data(data))
            }
            Action::Select { target, values } => self
                .execute_command(select_args(target, values)?)
                .await
                .map(|data| StepOutput::new("target options selected").with_data(data)),
            Action::Drag { source, target } => self.drag(source, target).await,
            Action::Press { key } => self
                .execute_command(vec!["press".into(), key.into()])
                .await
                .map(|data| StepOutput::new("key pressed").with_data(data)),
            Action::TerminalPaste { .. }
            | Action::TerminalResize { .. }
            | Action::TerminalRecording { .. } => Err(DriverError::new(
                "test.driver.web.action_unsupported",
                "terminal actions are available only on terminal surfaces",
            )),
            Action::Wheel {
                target,
                delta_x,
                delta_y,
                modifiers,
            } => {
                self.wheel(target.as_ref(), *delta_x, *delta_y, modifiers.as_slice())
                    .await
            }
            Action::Viewport {
                width,
                height,
                scale,
            } => {
                if *width == 0 || *height == 0 || scale == &Some(0) {
                    return Err(DriverError::new(
                        "test.driver.web.viewport_invalid",
                        "viewport width, height, and optional scale must be greater than zero",
                    ));
                }
                self.execute_command(viewport_args(*width, *height, *scale))
                    .await
                    .map(|data| StepOutput::new("viewport updated").with_data(data))
            }
            Action::Wait { condition } => {
                let args = wait_args(condition)?;
                self.execute_command(args)
                    .await
                    .map(|data| StepOutput::new("wait condition satisfied").with_data(data))
            }
            Action::Assert { expectation } => self.assert(expectation).await,
            Action::Screenshot { path } => self.screenshot(path).await,
            Action::Tab { operation } => self
                .execute_command(tab_args(operation))
                .await
                .map(|data| StepOutput::new("tab operation completed").with_data(data)),
            Action::Frame { target } => self
                .execute_command(frame_args(target))
                .await
                .map(|data| StepOutput::new("frame context changed").with_data(data)),
            Action::Dialog { operation } => self
                .execute_command(dialog_args(operation))
                .await
                .map(|data| StepOutput::new("dialog operation completed").with_data(data)),
            Action::Upload { target, paths } => {
                let args = upload_args(target, paths)?;
                self.execute_command(args)
                    .await
                    .map(|data| StepOutput::new("files uploaded").with_data(data))
            }
            Action::Download { target, path } => self.download(target, path).await,
            Action::NetworkRoute { pattern, route } => self
                .execute_command(network_route_args(pattern, route))
                .await
                .map(|data| StepOutput::new("network route installed").with_data(data)),
            Action::NetworkUnroute { pattern } => self
                .execute_command(network_unroute_args(pattern.as_deref()))
                .await
                .map(|data| StepOutput::new("network route removed").with_data(data)),
            Action::Har { operation } => self.har(operation).await,
            Action::Trace { operation } => self.trace(operation).await,
            Action::Video { operation } => self.video(operation).await,
            Action::Accessibility { path, interactive } => {
                let mut args = vec![OsString::from("snapshot")];
                if *interactive {
                    args.push(OsString::from("-i"));
                }
                self.capture_json(args, path, "accessibility snapshot captured")
                    .await
            }
            Action::Console { path, clear } => {
                let mut args = vec![OsString::from("console")];
                if *clear {
                    args.push(OsString::from("--clear"));
                }
                self.capture_json(args, path, "browser console captured")
                    .await
            }
            Action::PageErrors { path, clear } => {
                let mut args = vec![OsString::from("errors")];
                if *clear {
                    args.push(OsString::from("--clear"));
                }
                self.capture_json(args, path, "page errors captured").await
            }
            Action::VerifyContract { .. } => Err(DriverError::new(
                "test.driver.web.runner_action_unsupported",
                "verify_contract is executed by the A3S Test runner and must not reach a surface driver",
            )),
        }
    }

    async fn take_repairs(&mut self, limit: usize) -> Result<Vec<RepairFinding>, DriverError> {
        self.ensure_open()?;
        let bounded = limit.clamp(1, 50);
        let script = TAKE_REPAIRS_SCRIPT.replace("(50)", &format!("({bounded})"));
        let value = self
            .execute_command(vec!["eval".into(), script.into()])
            .await?;
        let mut findings: Vec<RepairFinding> = serde_json::from_value(browser_result(value))
            .map_err(|error| {
                DriverError::new(
                    "test.driver.web.repair_queue_invalid",
                    format!("Test Kit repair queue returned invalid findings: {error}"),
                )
            })?;
        materialize_design_references(&self.artifacts_dir, &mut findings).await?;
        Ok(findings)
    }

    async fn wait_for_repairs(
        &mut self,
        limit: usize,
        timeout_ms: u64,
        batch_window_ms: u64,
    ) -> Result<Vec<RepairFinding>, DriverError> {
        self.ensure_open()?;
        let bounded_limit = limit.clamp(1, 50);
        let command_budget_ms = u64::try_from(self.config.command_timeout.as_millis())
            .unwrap_or(u64::MAX)
            .saturating_sub(100)
            .max(1);
        let bounded_timeout = timeout_ms.min(300_000).min(command_budget_ms);
        let bounded_window = batch_window_ms.min(5_000).min(bounded_timeout);
        let script = WAIT_REPAIRS_SCRIPT
            .replace("limit: 50", &format!("limit: {bounded_limit}"))
            .replace("timeoutMs: 0", &format!("timeoutMs: {bounded_timeout}"))
            .replace(
                "batchWindowMs: 0",
                &format!("batchWindowMs: {bounded_window}"),
            );
        let value = self
            .execute_command(vec!["eval".into(), script.into()])
            .await?;
        let mut findings: Vec<RepairFinding> = serde_json::from_value(browser_result(value))
            .map_err(|error| {
                DriverError::new(
                    "test.driver.web.repair_queue_invalid",
                    format!("Test Kit repair watch returned invalid findings: {error}"),
                )
            })?;
        materialize_design_references(&self.artifacts_dir, &mut findings).await?;
        Ok(findings)
    }

    async fn apply_repair_event(&mut self, event: &RepairStatusEvent) -> Result<(), DriverError> {
        self.ensure_open()?;
        let event = serde_json::to_string(event).map_err(|error| {
            DriverError::new(
                "test.driver.web.repair_event_invalid",
                format!("failed to encode repair status event: {error}"),
            )
        })?;
        let script = format!(
            "(() => {{ const bridge = window[Symbol.for(\"a3s.test.page-context\")]; return bridge?.applyRepairEvent?.({event}) ?? null; }})()"
        );
        self.execute_command(vec!["eval".into(), script.into()])
            .await
            .map(drop)
    }

    async fn project_quality_report(
        &mut self,
        report: &ContractReport,
    ) -> Result<bool, DriverError> {
        self.ensure_open()?;
        let report = serde_json::to_string(report).map_err(|error| {
            DriverError::new(
                "test.driver.web.quality_report_invalid",
                format!("failed to encode the quality report: {error}"),
            )
        })?;
        let script = REPORT_QUALITY_SCRIPT.replace("(null)", &format!("({report})"));
        let value = self
            .execute_command(vec!["eval".into(), script.into()])
            .await?;
        Ok(browser_result(value).as_bool().unwrap_or(false))
    }

    async fn project_design_audit_report(
        &mut self,
        report: &DesignAuditReport,
    ) -> Result<bool, DriverError> {
        self.ensure_open()?;
        let report = serde_json::to_string(report).map_err(|error| {
            DriverError::new(
                "test.driver.web.design_audit_report_invalid",
                format!("failed to encode the design-audit report: {error}"),
            )
        })?;
        let script = REPORT_DESIGN_AUDIT_SCRIPT.replace("(null)", &format!("({report})"));
        let value = self
            .execute_command(vec!["eval".into(), script.into()])
            .await?;
        Ok(browser_result(value).as_bool().unwrap_or(false))
    }

    async fn take_repair_actions(
        &mut self,
        limit: usize,
    ) -> Result<Vec<RepairHumanAction>, DriverError> {
        self.ensure_open()?;
        let bounded = limit.clamp(1, 50);
        let script = TAKE_REPAIR_ACTIONS_SCRIPT.replace("(50)", &format!("({bounded})"));
        let value = self
            .execute_command(vec!["eval".into(), script.into()])
            .await?;
        serde_json::from_value(browser_result(value)).map_err(|error| {
            DriverError::new(
                "test.driver.web.repair_action_invalid",
                format!("Test Kit returned invalid human repair actions: {error}"),
            )
        })
    }

    async fn capture_repair_evidence(
        &mut self,
        request: &RepairEvidenceRequest,
    ) -> Result<RepairEvidenceBundle, DriverError> {
        self.ensure_open()?;
        let context = self.capture_page_context().await?;
        let snapshot = context.snapshot.ok_or_else(|| {
            DriverError::new(
                "test.driver.web.repair_evidence_context_missing",
                "repair evidence requires a compatible Test Kit context",
            )
        })?;
        let context_revision = snapshot.revision.ok_or_else(|| {
            DriverError::new(
                "test.driver.web.repair_evidence_context_invalid",
                "repair evidence context is missing its revision",
            )
        })?;
        let context_bytes = serde_json::to_vec(&snapshot).map_err(|error| {
            DriverError::new(
                "test.driver.web.repair_evidence_invalid",
                format!("failed to encode repair page context: {error}"),
            )
        })?;
        let phase = match request.phase {
            RepairEvidencePhase::Before => "before",
            RepairEvidencePhase::After => "after",
        };
        let attempt = request.attempt_id.as_deref().unwrap_or("submitted");
        validate_component(&request.finding_id, "finding id")?;
        validate_component(attempt, "attempt id")?;
        let requested = format!("repairs/{}/{attempt}/{phase}.png", request.finding_id);
        let screenshot_output = self.screenshot(&requested).await?;
        let screenshot = screenshot_output
            .evidence
            .into_iter()
            .next()
            .ok_or_else(|| {
                DriverError::new(
                    "test.driver.web.repair_evidence_invalid",
                    "repair screenshot did not produce evidence metadata",
                )
            })?;
        let screenshot_path = PathBuf::from(&screenshot.path);
        let screenshot_bytes = tokio::fs::read(&screenshot_path).await.map_err(|error| {
            DriverError::new(
                "test.driver.web.repair_evidence_invalid",
                format!("failed to read repair screenshot: {error}"),
            )
        })?;
        Ok(RepairEvidenceBundle {
            captured_at_ms: unix_ms(),
            context_revision,
            context_sha256: format!("{:x}", Sha256::digest(context_bytes)),
            context: snapshot,
            console_errors: self.page_console_error_count().await?,
            page_errors: self.page_error_count().await?,
            screenshot,
            screenshot_sha256: format!("{:x}", Sha256::digest(screenshot_bytes)),
        })
    }

    async fn prove_repair_acl(
        &mut self,
        finding_id: &str,
        attempt_id: &str,
        finding_url: &str,
        candidate: &str,
    ) -> Result<RepairAclProof, DriverError> {
        self.ensure_open()?;
        validate_component(finding_id, "finding id")?;
        validate_component(attempt_id, "attempt id")?;
        let suite = TestSuite::from_repair_acl(candidate, finding_url).map_err(|error| {
            DriverError::new(
                "test.driver.repair_acl_invalid",
                format!("repair ACL candidate is invalid: {}", error.message()),
            )
        })?;
        let requested = format!("repairs/{finding_id}/{attempt_id}/regression.acl");
        let path = prepare_artifact_path(&self.artifacts_dir, &requested).await?;
        tokio::fs::write(&path, candidate).await.map_err(|error| {
            DriverError::new(
                "test.driver.repair_acl_write_failed",
                format!("failed to persist repair ACL candidate: {error}"),
            )
        })?;
        validate_artifact_file(&self.artifacts_dir, &path).await?;

        let proof_context = ScenarioContext {
            run_id: format!("repair-proof-{}", compact_component(attempt_id, 24)),
            scenario_id: format!("proof-{}", compact_component(finding_id, 24)),
            artifacts_dir: self
                .artifacts_dir
                .join("repairs")
                .join(finding_id)
                .join(attempt_id)
                .join("proof"),
        };
        let driver =
            AgentBrowserDriver::with_executor(self.config.clone(), Arc::clone(&self.executor));
        let mut proof_session = driver.open(&proof_context).await?;
        let scenario = suite.scenarios.first().ok_or_else(|| {
            DriverError::new(
                "test.driver.repair_acl_invalid",
                "repair ACL candidate has no scenario",
            )
        })?;
        let mut failure = None;
        for step in &scenario.steps {
            if let Err(error) = proof_session.execute(step).await {
                failure = Some(format!(
                    "step '{}' failed with {}: {}",
                    step.id,
                    error.code(),
                    error.message()
                ));
                break;
            }
        }
        if let Err(error) = proof_session.close().await {
            let cleanup = format!("fresh proof browser cleanup failed: {}", error.message());
            failure =
                Some(failure.map_or(cleanup.clone(), |existing| format!("{existing}; {cleanup}")));
        }
        let passed = failure.is_none();
        Ok(RepairAclProof {
            path: requested,
            passed,
            summary: failure.unwrap_or_else(|| {
                "ACL candidate passed in a fresh browser session with the owning network policy"
                    .to_string()
            }),
        })
    }

    async fn validate_page_context_revision(
        &mut self,
        expected_revision: u64,
    ) -> Result<(), DriverError> {
        self.ensure_open()?;
        let current = self.capture_page_context().await?;
        if !current.present {
            return Err(DriverError::new(
                "test.driver.web.page_context_lost",
                "the Test Kit page context bridge is no longer present",
            ));
        }
        if current.revision != Some(expected_revision) {
            return Err(DriverError::new(
                "test.driver.web.page_context_stale",
                format!(
                    "page context revision changed from {expected_revision} to {}",
                    current
                        .revision
                        .map_or_else(|| "unknown".to_string(), |value| value.to_string())
                ),
            ));
        }
        Ok(())
    }

    async fn page_context_delta(
        &mut self,
        since_revision: u64,
    ) -> Result<Option<PageContextObservation>, DriverError> {
        if since_revision == 0 {
            return Err(DriverError::new(
                "test.driver.web.page_context_diff_invalid",
                "page context diff revision must be positive",
            ));
        }
        self.inspect_page_context(&PageContextInspectRequest {
            detail: "diff".to_string(),
            scope: PageContextInspectScope::Page,
            since_revision: Some(since_revision),
            wait_timeout_ms: 0,
            cursor: None,
            limit: 5_000,
        })
        .await
        .map(Some)
    }

    async fn inspect_page_context(
        &mut self,
        request: &PageContextInspectRequest,
    ) -> Result<PageContextObservation, DriverError> {
        self.ensure_open()?;
        if !matches!(
            request.detail.as_str(),
            "summary" | "scoped" | "diff" | "forensic"
        ) {
            return Err(DriverError::new(
                "test.driver.web.page_context_inspect_invalid",
                "page context detail must be summary, scoped, diff, or forensic",
            ));
        }
        if request.detail == "diff" {
            if request.since_revision.is_none_or(|revision| revision == 0) {
                return Err(DriverError::new(
                    "test.driver.web.page_context_diff_invalid",
                    "diff inspection requires a positive since revision",
                ));
            }
        } else if request.since_revision.is_some() || request.wait_timeout_ms != 0 {
            return Err(DriverError::new(
                "test.driver.web.page_context_diff_invalid",
                "since revision and diff wait are valid only for diff inspection",
            ));
        }
        if request.wait_timeout_ms > 300_000 {
            return Err(DriverError::new(
                "test.driver.web.page_context_diff_invalid",
                "page context diff wait cannot exceed 300000 milliseconds",
            ));
        }
        let scope = match &request.scope {
            PageContextInspectScope::Page => serde_json::json!({ "kind": "page" }),
            PageContextInspectScope::Node(node_id) => {
                serde_json::json!({ "kind": "node", "nodeId": node_id })
            }
            PageContextInspectScope::Component(component_id) => {
                serde_json::json!({ "kind": "component", "componentId": component_id })
            }
            PageContextInspectScope::Region {
                space,
                x,
                y,
                width,
                height,
            } => serde_json::json!({
                "kind": "region",
                "space": space,
                "x": x,
                "y": y,
                "width": width,
                "height": height,
            }),
        };
        let request_value = serde_json::json!({
            "detail": request.detail,
            "scope": scope,
            "sinceRevision": request.since_revision,
            "cursor": request.cursor,
            "ui": request.detail != "diff",
            "limits": { "nodes": request.limit.clamp(1, 5_000) },
        });
        let payload = serde_json::json!({
            "request": request_value,
            "waitTimeoutMs": request.wait_timeout_ms,
        });
        let script = format!("{PAGE_CONTEXT_INSPECT_FUNCTION}({payload})");
        let value = self
            .execute_command(vec!["eval".into(), script.into()])
            .await?;
        let observation = parse_page_context_value(browser_result(value))?;
        validate_inspect_response(request, &observation)?;
        Ok(observation)
    }

    async fn page_console_error_count(&mut self) -> Result<u32, DriverError> {
        self.ensure_open()?;
        let value = self.execute_command(vec!["console".into()]).await?;
        Ok(count_error_entries(&browser_result(value)))
    }

    async fn page_error_count(&mut self) -> Result<u32, DriverError> {
        self.ensure_open()?;
        let value = self.execute_command(vec!["errors".into()]).await?;
        Ok(count_collection_entries(&browser_result(value)))
    }

    async fn capture_grounding_screenshot(
        &mut self,
        requested_path: &str,
        expected_surface_revision: Option<u64>,
    ) -> Result<GroundingScreenshot, DriverError> {
        self.ensure_open()?;
        if PathBuf::from(requested_path)
            .extension()
            .and_then(|extension| extension.to_str())
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("png"))
        {
            return Err(DriverError::new(
                "test.driver.web.grounding_screenshot_invalid",
                "grounding screenshots must use a .png artifact path",
            ));
        }
        let before = self.capture_page_context().await?;
        validate_grounding_revision(&before, expected_surface_revision)?;
        let screenshot_output = self.screenshot(requested_path).await?;
        let after = self.capture_page_context().await?;
        validate_grounding_revision(&after, expected_surface_revision)?;
        if !stable_page_context(&before, &after) {
            return Err(DriverError::new(
                "test.driver.web.page_context_changed",
                "page context changed while the grounding screenshot was captured",
            ));
        }
        let evidence = screenshot_output
            .evidence
            .into_iter()
            .next()
            .ok_or_else(|| {
                DriverError::new(
                    "test.driver.web.grounding_screenshot_invalid",
                    "grounding screenshot did not produce evidence metadata",
                )
            })?;
        let bytes = read_bounded_artifact(
            &self.artifacts_dir,
            Path::new(&evidence.path),
            MAX_GROUNDING_IMAGE_BYTES,
        )
        .await?;
        let (width, height) = png_dimensions(&bytes)?;
        Ok(GroundingScreenshot {
            evidence,
            sha256: format!("sha256:{:x}", Sha256::digest(bytes)),
            width,
            height,
            surface_revision: after.revision,
        })
    }

    async fn close(&mut self) -> Result<(), DriverError> {
        if self.closed {
            return Ok(());
        }

        if self.active_video.is_some() {
            let _ = self
                .execute_command(vec![OsString::from("record"), OsString::from("stop")])
                .await;
            self.active_video = None;
        }

        let graceful = self.execute_command(vec![OsString::from("close")]).await;
        let containment = self.terminate_registered_processes().await;
        let emergency_terminated = self.emergency_cleanup().await;
        let contained = containment?;
        match graceful {
            Ok(_) => {
                self.closed = true;
                Ok(())
            }
            Err(_) if contained || emergency_terminated => {
                self.closed = true;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }
}
