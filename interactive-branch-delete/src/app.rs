use std::cmp::Ordering;
use std::time::{Duration, SystemTime};

use crate::git::BranchInfo;

pub struct BranchItem {
    pub info: BranchInfo,
    pub selected: bool,
    pub age: Option<Duration>,
}

impl BranchItem {
    fn new(info: BranchInfo, now: SystemTime) -> Self {
        let age = info.age(now);
        Self {
            info,
            selected: false,
            age,
        }
    }
}

pub struct App {
    branches: Vec<BranchItem>,
    cursor: usize,
    should_quit: bool,
    confirmed: bool,
    message: Option<String>,
    base_branch: String,
    current_branch: String,
}

impl App {
    pub fn new(branches: Vec<BranchInfo>, base_branch: String, current_branch: String) -> Self {
        let now = SystemTime::now();
        let mut items: Vec<BranchItem> = branches
            .into_iter()
            .map(|info| BranchItem::new(info, now))
            .collect();

        items.sort_by(|a, b| match (&a.age, &b.age) {
            (Some(a_age), Some(b_age)) => b_age.cmp(a_age),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => a.info.name.cmp(&b.info.name),
        });

        Self {
            branches: items,
            cursor: 0,
            should_quit: false,
            confirmed: false,
            message: None,
            base_branch,
            current_branch,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.branches.is_empty()
    }

    pub fn move_down(&mut self) {
        if self.branches.is_empty() {
            return;
        }
        self.clear_message();
        self.cursor = (self.cursor + 1).min(self.branches.len() - 1);
    }

    pub fn move_up(&mut self) {
        if self.branches.is_empty() {
            return;
        }
        self.clear_message();
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn toggle_current(&mut self) {
        if let Some(current) = self.branches.get_mut(self.cursor) {
            current.selected = !current.selected;
        }
    }

    pub fn toggle_all(&mut self) {
        let all_selected = self.branches.iter().all(|branch| branch.selected);
        for branch in &mut self.branches {
            branch.selected = !all_selected;
        }
    }

    pub fn cancel(&mut self) {
        self.should_quit = true;
    }

    pub fn confirm(&mut self) {
        if self.selected_count() == 0 {
            self.set_message("Select at least one branch before confirming.");
            return;
        }
        self.confirmed = true;
        self.should_quit = true;
    }

    pub fn should_quit(&self) -> bool {
        self.should_quit
    }

    pub fn confirmed(&self) -> bool {
        self.confirmed
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn items(&self) -> &[BranchItem] {
        &self.branches
    }

    pub fn selected_count(&self) -> usize {
        self.branches
            .iter()
            .filter(|branch| branch.selected)
            .count()
    }

    pub fn total_count(&self) -> usize {
        self.branches.len()
    }

    pub fn set_message<S: Into<String>>(&mut self, message: S) {
        self.message = Some(message.into());
    }

    pub fn clear_message(&mut self) {
        self.message = None;
    }

    pub fn message(&self) -> Option<&str> {
        self.message.as_deref()
    }

    pub fn selected_branch_infos(&self) -> Vec<BranchInfo> {
        self.branches
            .iter()
            .filter(|branch| branch.selected)
            .map(|branch| branch.info.clone())
            .collect()
    }

    pub fn base_branch(&self) -> &str {
        &self.base_branch
    }

    pub fn current_branch(&self) -> &str {
        &self.current_branch
    }
}
