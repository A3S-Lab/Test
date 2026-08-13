use std::path::PathBuf;

use a3s_test_core::ModifierKey;
use clap::{Args, Subcommand, ValueEnum};

use super::super::BrowserDriverKind;

#[derive(Debug, Args)]
pub(crate) struct AgentArgs {
    #[command(subcommand)]
    pub(super) command: AgentCommand,
}

#[derive(Debug, Subcommand)]
pub(super) enum AgentCommand {
    /// Run one bounded Web test with a deployment-supplied LLM provider.
    Run(crate::agent_host::AgentRunArgs),
    /// Start a persistent Web test session for an external coding agent.
    #[command(alias = "open")]
    Start(StartArgs),
    /// Capture the next semantic observation from an active session.
    #[command(alias = "snapshot")]
    Observe(ObserveArgs),
    /// Inspect one bounded Test Kit page, component, node, or region scope.
    Inspect(InspectArgs),
    /// Locate a target from a fresh revision-bound screenshot without acting.
    Ground(GroundArgs),
    /// Execute one schema-validated action in an active session.
    Act(ActArgs),
    /// Click a ref or CSS target in an active session.
    Click(TargetArgs),
    /// Hover a target in an active session.
    Hover(TargetArgs),
    /// Focus a ref or CSS target in an active session.
    Focus(TargetArgs),
    /// Double-click a ref or CSS target in an active session.
    #[command(alias = "dblclick")]
    DoubleClick(TargetArgs),
    /// Open the context menu for a ref or CSS target.
    #[command(alias = "right-click")]
    ContextClick(TargetArgs),
    /// Replace the value of a ref or CSS target in an active session.
    Fill(TargetValueArgs),
    /// Type without clearing a ref or CSS target.
    Type(TargetValueArgs),
    /// Check a target in an active session.
    Check(TargetArgs),
    /// Uncheck a ref or CSS target in an active session.
    Uncheck(TargetArgs),
    /// Select one or more values in a ref or CSS target.
    Select(SelectArgs),
    /// Drag one ref or CSS target to another.
    Drag(DragArgs),
    /// Send one key or key chord to the active session.
    Press(PressArgs),
    /// Dispatch a mouse wheel gesture with optional held modifiers.
    Wheel(WheelArgs),
    /// Set the browser viewport.
    Viewport(ViewportArgs),
    /// Capture a screenshot inside the session artifact directory.
    Screenshot(ScreenshotArgs),
    /// Pick up queued Test Kit findings and persist them in this session.
    RepairWatch(RepairWatchArgs),
    /// Claim one queued repair finding.
    RepairClaim(RepairTransitionArgs),
    /// Report that workspace editing has started.
    RepairProgress(RepairTransitionArgs),
    /// Request human clarification.
    RepairReply(RepairTransitionArgs),
    /// Report editing complete and begin A3S Test verification.
    RepairComplete(RepairTransitionArgs),
    /// Verify a completed repair against a newer ready page revision.
    RepairVerify(RepairVerifyArgs),
    /// Record a failed repair attempt.
    RepairFail(RepairTransitionArgs),
    /// Cancel a queued or claimed repair.
    RepairCancel(RepairTransitionArgs),
    /// Finish the session, close its surface, and write a report.
    Finish(FinishArgs),
    /// Abort an active session and close only its owned surface.
    Abort(SessionArgs),
    /// Show the persisted state for one session.
    Show(SessionArgs),
    /// List sessions in the current workspace.
    List(ListArgs),
    /// Print the external-planner protocol and typed action schema.
    Schema(SchemaArgs),
}

#[derive(Debug, Args)]
pub(super) struct StartArgs {
    /// Initial Web URL.
    pub(super) url: String,
    /// Stable workspace-local session identifier.
    #[arg(long)]
    pub(super) session: String,
    /// Concrete test goal for the coding agent.
    #[arg(long)]
    pub(super) goal: String,
    /// Observable success criterion. Repeat for multiple criteria.
    #[arg(long = "success", required = true)]
    pub(super) success_criteria: Vec<String>,
    /// Automatically resolve repairs only after every verification gate passes.
    #[arg(long)]
    pub(super) auto_resolve_repairs: bool,
    /// Additional navigation origin allowed during this session.
    #[arg(long = "allow-origin")]
    pub(super) allowed_origins: Vec<String>,
    /// Additional hostname allowed by the browser network filter, not the A3S origin gate.
    #[arg(long = "allow-domain")]
    pub(super) allowed_domains: Vec<String>,
    /// Browser driver integration.
    #[arg(long, value_enum, default_value_t = BrowserDriverKind::A3s)]
    pub(super) browser_driver: BrowserDriverKind,
    /// Override the browser driver executable.
    #[arg(long)]
    pub(super) browser_executable: Option<PathBuf>,
    /// Show the browser window; omitted runs enforce headless execution.
    #[arg(long)]
    pub(super) headed: bool,
    /// Per-command browser deadline.
    #[arg(long, default_value_t = 25_000)]
    pub(super) command_timeout_ms: u64,
    /// Browser daemon inactivity deadline between agent turns.
    #[arg(long, default_value_t = 300_000)]
    pub(super) idle_timeout_ms: u64,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct ObserveArgs {
    /// Active session identifier.
    #[arg(long)]
    pub(super) session: String,
    /// Return only interactive elements when supported.
    #[arg(long)]
    pub(super) interactive: bool,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct InspectArgs {
    /// Active session identifier.
    #[arg(long)]
    pub(super) session: String,
    /// Context detail profile.
    #[arg(long, default_value = "scoped", value_parser = ["summary", "scoped", "diff", "forensic"])]
    pub(super) detail: String,
    /// Current private Test Kit node ID. Prefer component or region scopes for persisted workflows.
    #[arg(long, conflicts_with_all = ["component", "region"])]
    pub(super) node: Option<String>,
    /// Registered Test Kit component ID.
    #[arg(long, conflicts_with_all = ["node", "region"])]
    pub(super) component: Option<String>,
    /// Region as `space,x,y,width,height`, where space is viewport or document.
    #[arg(long, conflicts_with_all = ["node", "component"])]
    pub(super) region: Option<String>,
    /// Opaque pagination cursor returned by a previous inspection.
    #[arg(long)]
    pub(super) cursor: Option<String>,
    /// Maximum returned nodes.
    #[arg(long, default_value_t = 100)]
    pub(super) limit: usize,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct GroundArgs {
    /// Natural-language description of the target to locate.
    pub(super) query: String,
    /// Active session identifier.
    #[arg(long)]
    pub(super) session: String,
    /// Latest observation identifier whose page revision is being grounded.
    #[arg(long)]
    pub(super) observation: u64,
    /// ACL provider configuration.
    #[arg(long)]
    pub(super) config: PathBuf,
    /// Typed reason for using visual fallback.
    #[arg(long, value_enum, default_value_t = GroundingReason::Explicit)]
    pub(super) reason: GroundingReason,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum GroundingReason {
    Explicit,
    Canvas,
    ImageOnly,
    RemoteDesktop,
    DesignReference,
    NoSemanticMatch,
}

#[derive(Debug, Args)]
pub(super) struct ActArgs {
    /// Active session identifier.
    #[arg(long)]
    pub(super) session: String,
    /// One JSON object matching the Action schema.
    #[arg(long = "action-json")]
    pub(super) action_json: String,
    /// Observation identifier that supplied any ref target used by the action.
    #[arg(long)]
    pub(super) observation: Option<u64>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct TargetArgs {
    /// Observation ref such as @e3 or @c2, or an explicit CSS selector.
    pub(super) target: String,
    /// Active session identifier.
    #[arg(long)]
    pub(super) session: String,
    /// Observation identifier that supplied a ref target.
    #[arg(long)]
    pub(super) observation: Option<u64>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct TargetValueArgs {
    /// Observation ref such as @e3 or @c2, or an explicit CSS selector.
    pub(super) target: String,
    /// Text value.
    pub(super) value: String,
    /// Active session identifier.
    #[arg(long)]
    pub(super) session: String,
    /// Observation identifier that supplied a ref target.
    #[arg(long)]
    pub(super) observation: Option<u64>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct SelectArgs {
    /// Ref such as @e3, or an explicit CSS selector.
    pub(super) target: String,
    /// One or more option values.
    #[arg(required = true, num_args = 1..)]
    pub(super) values: Vec<String>,
    /// Active session identifier.
    #[arg(long)]
    pub(super) session: String,
    /// Observation identifier that supplied a ref target.
    #[arg(long)]
    pub(super) observation: Option<u64>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct DragArgs {
    /// Source ref or CSS selector.
    pub(super) source: String,
    /// Destination ref or CSS selector.
    pub(super) target: String,
    /// Active session identifier.
    #[arg(long)]
    pub(super) session: String,
    /// Observation identifier that supplied either ref target.
    #[arg(long)]
    pub(super) observation: Option<u64>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct PressArgs {
    /// Key or key chord, for example Enter or Meta+z.
    pub(super) key: String,
    /// Active session identifier.
    #[arg(long)]
    pub(super) session: String,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum ModifierArg {
    Alt,
    Control,
    Meta,
    Shift,
}

impl From<ModifierArg> for ModifierKey {
    fn from(value: ModifierArg) -> Self {
        match value {
            ModifierArg::Alt => Self::Alt,
            ModifierArg::Control => Self::Control,
            ModifierArg::Meta => Self::Meta,
            ModifierArg::Shift => Self::Shift,
        }
    }
}

#[derive(Debug, Args)]
pub(super) struct WheelArgs {
    /// Vertical wheel delta. Negative values scroll or zoom in.
    #[arg(allow_hyphen_values = true)]
    pub(super) delta_y: i32,
    /// Horizontal wheel delta.
    #[arg(long, default_value_t = 0, allow_hyphen_values = true)]
    pub(super) delta_x: i32,
    /// Optional ref or CSS target for a synthetic target-scoped wheel event.
    #[arg(long)]
    pub(super) target: Option<String>,
    /// Modifier to hold for the gesture. Repeat for multiple modifiers.
    #[arg(long = "modifier", value_enum)]
    pub(super) modifiers: Vec<ModifierArg>,
    /// Active session identifier.
    #[arg(long)]
    pub(super) session: String,
    /// Observation identifier that supplied a ref target.
    #[arg(long)]
    pub(super) observation: Option<u64>,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct ViewportArgs {
    /// Viewport width in CSS pixels.
    pub(super) width: u32,
    /// Viewport height in CSS pixels.
    pub(super) height: u32,
    /// Optional integer device scale factor.
    #[arg(long)]
    pub(super) scale: Option<u32>,
    /// Active session identifier.
    #[arg(long)]
    pub(super) session: String,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct ScreenshotArgs {
    /// Relative path below the session artifact directory.
    pub(super) path: String,
    /// Active session identifier.
    #[arg(long)]
    pub(super) session: String,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct RepairWatchArgs {
    #[arg(long)]
    pub(super) session: String,
    #[arg(long, default_value_t = 20)]
    pub(super) limit: usize,
    /// Maximum time to wait for a page submission.
    #[arg(long, default_value_t = 30_000)]
    pub(super) timeout_ms: u64,
    /// Short window used to collect findings submitted together.
    #[arg(long, default_value_t = 250)]
    pub(super) batch_window_ms: u64,
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct RepairTransitionArgs {
    pub(super) finding_id: String,
    #[arg(long)]
    pub(super) session: String,
    #[arg(long)]
    pub(super) request_id: String,
    #[arg(long)]
    pub(super) attempt_id: Option<String>,
    #[arg(long)]
    pub(super) lease_expires_at_ms: Option<u64>,
    /// Lease duration from now. Used by claim when no absolute expiry is supplied.
    #[arg(long, default_value_t = 300_000)]
    pub(super) lease_ms: u64,
    #[arg(long)]
    pub(super) summary: Option<String>,
    #[arg(long)]
    pub(super) message: Option<String>,
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct RepairVerifyArgs {
    pub(super) finding_id: String,
    #[arg(long)]
    pub(super) session: String,
    #[arg(long)]
    pub(super) request_id: String,
    #[arg(long)]
    pub(super) success_criteria_passed: Option<bool>,
    #[arg(long = "changed-file")]
    pub(super) changed_files: Vec<String>,
    /// JSON array of `{command,status,summary}` focused check results.
    #[arg(long, default_value = "[]")]
    pub(super) checks_json: String,
    #[arg(long)]
    pub(super) acl_candidate: Option<String>,
    #[arg(long)]
    pub(super) summary: String,
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(super) enum FinishStatus {
    Passed,
    Failed,
}

#[derive(Debug, Args)]
pub(super) struct FinishArgs {
    /// Active session identifier.
    #[arg(long)]
    pub(super) session: String,
    /// Final test status decided from explicit success criteria and evidence.
    #[arg(long, value_enum)]
    pub(super) status: FinishStatus,
    /// Concise result summary.
    #[arg(long)]
    pub(super) summary: String,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct SessionArgs {
    /// Session identifier.
    #[arg(long)]
    pub(super) session: String,
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct ListArgs {
    /// Emit machine-readable JSON.
    #[arg(long)]
    pub(super) json: bool,
}

#[derive(Debug, Args)]
pub(super) struct SchemaArgs {
    /// Emit compact JSON instead of pretty JSON.
    #[arg(long)]
    pub(super) compact: bool,
}
