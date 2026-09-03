use std::collections::HashSet;
use std::path::PathBuf;

use crate::scanner::OrphanedPrefix;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppState {
    Browsing,
    ConfirmingDeletion,
    Cleaning,
    Done,
}

pub struct App {
    pub orphans: Vec<OrphanedPrefix>,
    pub selected_appids: HashSet<String>,
    pub cursor_index: usize,
    pub state: AppState,
    pub animation_frame: usize,
    pub status_message: String,
    pub backup_dir: PathBuf,
    pub space_reclaimed: u64,
    pub should_quit: bool,
}

impl App {
    pub fn new(orphans: Vec<OrphanedPrefix>, backup_dir: PathBuf) -> Self {
        let mut selected_appids = HashSet::new();
        // By default, select all detected orphans
        for o in &orphans {
            selected_appids.insert(o.appid.clone());
        }

        Self {
            orphans,
            selected_appids,
            cursor_index: 0,
            state: AppState::Browsing,
            animation_frame: 0,
            status_message: "Ready. Use [Space] to toggle, [c] to clean, [q] to quit.".to_string(),
            backup_dir,
            space_reclaimed: 0,
            should_quit: false,
        }
    }

    pub fn tick(&mut self) {
        self.animation_frame = (self.animation_frame + 1) % 60;
    }

    pub fn toggle_selection(&mut self) {
        if let Some(orphan) = self.orphans.get(self.cursor_index) {
            if self.selected_appids.contains(&orphan.appid) {
                self.selected_appids.remove(&orphan.appid);
            } else {
                self.selected_appids.insert(orphan.appid.clone());
            }
        }
    }

    pub fn toggle_all(&mut self) {
        if self.selected_appids.len() == self.orphans.len() {
            self.selected_appids.clear();
        } else {
            for o in &self.orphans {
                self.selected_appids.insert(o.appid.clone());
            }
        }
    }

    pub fn next_item(&mut self) {
        if !self.orphans.is_empty() {
            self.cursor_index = (self.cursor_index + 1) % self.orphans.len();
        }
    }

    pub fn prev_item(&mut self) {
        if !self.orphans.is_empty() {
            if self.cursor_index == 0 {
                self.cursor_index = self.orphans.len() - 1;
            } else {
                self.cursor_index -= 1;
            }
        }
    }

    pub fn selected_total_size(&self) -> u64 {
        self.orphans
            .iter()
            .filter(|o| self.selected_appids.contains(&o.appid))
            .map(|o| o.total_size())
            .sum()
    }
}
