use std::collections::HashSet;
use std::path::PathBuf;

use crate::scanner::OrphanedPrefix;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppState {
    Browsing,
    Filtering,
    ConfirmingDeletion,
    ShowingHelp,
    Cleaning,
    Done,
}

pub struct App {
    pub all_orphans: Vec<OrphanedPrefix>,
    pub filtered_indices: Vec<usize>,
    pub filter_query: String,
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
        for o in &orphans {
            selected_appids.insert(o.appid.clone());
        }

        let count = orphans.len();
        let filtered_indices = (0..count).collect();

        Self {
            all_orphans: orphans,
            filtered_indices,
            filter_query: String::new(),
            selected_appids,
            cursor_index: 0,
            state: AppState::Browsing,
            animation_frame: 0,
            status_message:
                "Ready. [Space] Select | [a] All | [c] Clean | [/] Filter | [?] Help | [q] Quit"
                    .to_string(),
            backup_dir,
            space_reclaimed: 0,
            should_quit: false,
        }
    }

    pub fn tick(&mut self) {
        self.animation_frame = (self.animation_frame + 1) % 120;
    }

    pub fn current_orphan(&self) -> Option<&OrphanedPrefix> {
        self.filtered_indices
            .get(self.cursor_index)
            .and_then(|&idx| self.all_orphans.get(idx))
    }

    pub fn apply_filter(&mut self) {
        let q = self.filter_query.to_lowercase();
        self.filtered_indices = self
            .all_orphans
            .iter()
            .enumerate()
            .filter(|(_, o)| {
                if q.is_empty() {
                    true
                } else {
                    o.appid.contains(&q)
                        || o.title
                            .as_ref()
                            .map(|t| t.to_lowercase().contains(&q))
                            .unwrap_or(false)
                }
            })
            .map(|(idx, _)| idx)
            .collect();

        if self.cursor_index >= self.filtered_indices.len() {
            self.cursor_index = self.filtered_indices.len().saturating_sub(1);
        }
    }

    pub fn toggle_selection(&mut self) {
        if let Some(orphan) = self.current_orphan() {
            let appid = orphan.appid.clone();
            if self.selected_appids.contains(&appid) {
                self.selected_appids.remove(&appid);
            } else {
                self.selected_appids.insert(appid);
            }
        }
    }

    pub fn toggle_all(&mut self) {
        let visible_appids: Vec<String> = self
            .filtered_indices
            .iter()
            .filter_map(|&idx| self.all_orphans.get(idx))
            .map(|o| o.appid.clone())
            .collect();

        let all_selected = visible_appids
            .iter()
            .all(|id| self.selected_appids.contains(id));

        if all_selected {
            for id in visible_appids {
                self.selected_appids.remove(&id);
            }
        } else {
            for id in visible_appids {
                self.selected_appids.insert(id);
            }
        }
    }

    pub fn invert_selection(&mut self) {
        for &idx in &self.filtered_indices {
            if let Some(orphan) = self.all_orphans.get(idx) {
                if self.selected_appids.contains(&orphan.appid) {
                    self.selected_appids.remove(&orphan.appid);
                } else {
                    self.selected_appids.insert(orphan.appid.clone());
                }
            }
        }
    }

    pub fn next_item(&mut self) {
        if !self.filtered_indices.is_empty() {
            self.cursor_index = (self.cursor_index + 1) % self.filtered_indices.len();
        }
    }

    pub fn prev_item(&mut self) {
        if !self.filtered_indices.is_empty() {
            if self.cursor_index == 0 {
                self.cursor_index = self.filtered_indices.len() - 1;
            } else {
                self.cursor_index -= 1;
            }
        }
    }

    pub fn selected_total_size(&self) -> u64 {
        self.all_orphans
            .iter()
            .filter(|o| self.selected_appids.contains(&o.appid))
            .map(|o| o.total_size())
            .sum()
    }

    pub fn total_orphans_size(&self) -> u64 {
        self.all_orphans.iter().map(|o| o.total_size()).sum()
    }
}
