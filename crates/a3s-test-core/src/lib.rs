//! Typed test specifications and surface-driver contracts for A3S Test.

mod driver;
mod error;
mod manifest;
mod model;

pub use driver::{DriverSession, ScenarioContext, SurfaceDriver};
pub use error::{DriverError, SpecError};
pub use model::{
    Action, CaptureOperation, DialogOperation, Evidence, Expectation, FrameTarget, LoadState,
    NetworkRoute, StepOutput, Surface, SurfaceObservation, TabOperation, Target, TestScenario,
    TestStep, TestSuite, VideoOperation, WaitCondition,
};
