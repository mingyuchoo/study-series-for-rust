use crate::{Layout,
            ProjectLock,
            write_atomic};
use gm_application::{PortResult,
                     RunStateRepository};
use gm_core::RunState;

#[derive(Debug, Clone)]
pub struct FileRunState {
    layout: Layout,
}

impl FileRunState {
    pub fn new(layout: Layout) -> Self {
        Self {
            layout,
        }
    }
}

impl RunStateRepository for FileRunState {
    fn read(&self) -> Option<RunState> {
        let text = std::fs::read_to_string(self.layout.run_state()).ok()?;
        serde_json::from_str(&text).ok()
    }

    fn write(&self, state: &RunState) -> PortResult<()> {
        let text = serde_json::to_string_pretty(state)?;
        Ok(write_atomic(&self.layout.run_state(), text.as_bytes())?)
    }

    fn clear(&self) { let _ = std::fs::remove_file(self.layout.run_state()); }

    fn clear_if_matches(&self, state: &RunState) -> PortResult<()> {
        let _lock = ProjectLock::acquire_wait(&self.layout.lock_file())?;
        if let Some(current) = self.read()
            && current.pid == state.pid
            && current.process_start == state.process_start
            && current.started_at == state.started_at
        {
            self.clear();
        }
        Ok(())
    }
}
