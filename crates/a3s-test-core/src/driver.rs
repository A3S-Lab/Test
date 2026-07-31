use std::path::PathBuf;

use async_trait::async_trait;

use crate::{DriverError, StepOutput, Surface, TestStep};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScenarioContext {
    pub run_id: String,
    pub scenario_id: String,
    pub artifacts_dir: PathBuf,
}

#[async_trait]
pub trait DriverSession: Send {
    async fn execute(&mut self, step: &TestStep) -> Result<StepOutput, DriverError>;

    async fn close(&mut self) -> Result<(), DriverError>;
}

#[async_trait]
pub trait SurfaceDriver: Send + Sync {
    fn surface(&self) -> Surface;

    async fn open(&self, context: &ScenarioContext) -> Result<Box<dyn DriverSession>, DriverError>;
}
