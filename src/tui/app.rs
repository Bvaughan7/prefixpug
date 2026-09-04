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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    Size,
    Age,
    AppId,
}

pub struct App {
    pub all_orphans: Vec<OrphanedPrefix>,
    pub filtered_indices: Vec<usize>,
    pub filter_query: String,
    pub selected_appids: HashSet<String>,
    pub cursor_index: usize,
    pub state: AppState,
    pub sort_mode: SortMode,
    pub show_mascot: bool, // Togglable with 'm' (off by default for maximum data density)
    pub animation_frame: usize,
    pub status_message: String,
    pub backup_dir: PathBuf,
    pub space_reclaimed: u64,
    pub should_quit: bool,
}

impl App {
    pub fn new(orphans: Vec<OrphanedPrefix>, backup_dir: PathBuf) -> Self {
        let is_empty = orphans.is_empty();
        let selected_appids: HashSet<String> = orphans
            .iter()
            .filter(|o| o.is_deletable())
            .map(|o| o.appid.clone())
            .collect();
        let filtered_indices: Vec<usize> = (0..orphans.len()).collect();

        Self {
            all_orphans: orphans,
            filtered_indices,
            filter_query: String::new(),
            selected_appids,
            cursor_index: 0,
            state: AppState::Browsing,
            sort_mode: SortMode::Size,
            show_mascot: true, // Cyberpug mascot sentry enabled by default
            animation_frame: 0,
            status_message: if is_empty {
                "Storage clean. 0 orphaned prefixes detected.".to_string()
            } else {
                "Ready. [Space] to select, [c] to review and clean.".to_string()
            },
            backup_dir,
            space_reclaimed: 0,
            should_quit: false,
        }
    }

    pub fn tick(&mut self) {
        self.animation_frame = (self.animation_frame + 1) % 120;
    }

    pub fn toggle_mascot(&mut self) {
        self.show_mascot = !self.show_mascot;
        self.status_message = if self.show_mascot {
            "Mascot display enabled. Press [m] to hide."
        } else {
            "Data-dense layout active. Press [m] to show mascot."
        }
        .to_string();
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

    pub fn toggle_sort(&mut self) {
        self.sort_mode = match self.sort_mode {
            SortMode::Size => SortMode::Age,
            SortMode::Age => SortMode::AppId,
            SortMode::AppId => SortMode::Size,
        };

        match self.sort_mode {
            SortMode::Size => {
                self.all_orphans
                    .sort_by_key(|a| std::cmp::Reverse(a.total_size()));
                self.status_message = "Sorted by size (descending)".to_string();
            }
            SortMode::Age => {
                self.all_orphans
                    .sort_by_key(|a| a.last_modified.unwrap_or(std::time::UNIX_EPOCH));
                self.status_message = "Sorted by age (oldest first)".to_string();
            }
            SortMode::AppId => {
                self.all_orphans.sort_by(|a, b| a.appid.cmp(&b.appid));
                self.status_message = "Sorted by AppID".to_string();
            }
        }
        self.apply_filter();
    }

    pub fn toggle_selection(&mut self) {
        if let Some(orphan) = self.current_orphan() {
            if !orphan.is_deletable() {
                self.status_message = format!(
                    "{} is protected and cannot be selected for deletion.",
                    orphan.display_name()
                );
                return;
            }

            let appid = orphan.appid.clone();
            if self.selected_appids.contains(&appid) {
                self.selected_appids.remove(&appid);
            } else {
                self.selected_appids.insert(appid);
            }
        }
    }

    pub fn toggle_all(&mut self) {
        let selectable_visible_appids: Vec<String> = self
            .filtered_indices
            .iter()
            .filter_map(|&idx| self.all_orphans.get(idx))
            .filter(|o| o.is_deletable())
            .map(|o| o.appid.clone())
            .collect();

        let all_selected = selectable_visible_appids
            .iter()
            .all(|id| self.selected_appids.contains(id));

        if all_selected {
            for id in selectable_visible_appids {
                self.selected_appids.remove(&id);
            }
        } else {
            for id in selectable_visible_appids {
                self.selected_appids.insert(id);
            }
        }
    }

    pub fn invert_selection(&mut self) {
        for &idx in &self.filtered_indices {
            if let Some(orphan) = self.all_orphans.get(idx) {
                if !orphan.is_deletable() {
                    continue;
                }
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
        self.all_orphans
            .iter()
            .filter(|o| o.is_deletable())
            .map(|o| o.total_size())
            .sum()
    }
}
