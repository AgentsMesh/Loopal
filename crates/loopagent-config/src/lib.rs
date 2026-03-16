pub mod loader;
pub mod locations;
pub mod skills;

pub use loader::{load_instructions, load_settings};
pub use locations::*;
pub use skills::{Skill, load_skills};
