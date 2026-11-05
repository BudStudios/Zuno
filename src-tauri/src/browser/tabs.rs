//! Tab and tab-group model. Pure data structure with no webview dependencies so its invariants
//! can be unit tested:
//! * tab ids are unique and never reused within a run,
//! * tabs belonging to a group are always contiguous in the strip,
//! * groups without tabs are removed automatically.

use serde::Serialize;

use crate::storage::session::{Session, SessionGroup, SessionTab};

pub type TabId = u64;
pub type GroupId = u64;

/// Muted group tones (rendered as subtle tints in the UI, not saturated colours).
pub const GROUP_COLORS: &[&str] = &["slate", "sage", "sand", "clay", "mist", "plum"];

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Tab {
    pub id: TabId,
    pub url: String,
    pub title: String,
    pub favicon: Option<String>,
    pub loading: bool,
    pub can_go_back: bool,
    pub can_go_forward: bool,
    pub group: Option<GroupId>,
    pub error: Option<String>,
    /// Whether a webview currently exists for this tab (restored tabs are created lazily).
    pub live: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Group {
    pub id: GroupId,
    pub name: String,
    pub color: String,
    pub collapsed: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub tabs: Vec<Tab>,
    pub groups: Vec<Group>,
    pub active: Option<TabId>,
}

#[derive(Debug, Default)]
pub struct TabManager {
    tabs: Vec<Tab>,
    groups: Vec<Group>,
    active: Option<TabId>,
    /// Most-recently-used order (front = most recent), used to pick the next tab after closing.
    mru: Vec<TabId>,
    next_tab: TabId,
    next_group: GroupId,
}

impl TabManager {
    pub fn new() -> Self {
        Self { next_tab: 1, next_group: 1, ..Default::default() }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot { tabs: self.tabs.clone(), groups: self.groups.clone(), active: self.active }
    }

    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    pub fn active(&self) -> Option<TabId> {
        self.active
    }

    pub fn get(&self, id: TabId) -> Option<&Tab> {
        self.tabs.iter().find(|t| t.id == id)
    }

    pub fn get_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn index_of(&self, id: TabId) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    pub fn group(&self, id: GroupId) -> Option<&Group> {
        self.groups.iter().find(|g| g.id == id)
    }

    /// Adds a tab. `after` places it immediately to the right of that tab (inheriting its group,
    /// like links opened from a grouped tab); otherwise it is appended at the end.
    pub fn add(&mut self, url: &str, title: &str, after: Option<TabId>, activate: bool) -> TabId {
        let id = self.next_tab;
        self.next_tab += 1;
        let (index, group) = match after.and_then(|a| self.index_of(a).map(|i| (i, self.tabs[i].group))) {
            Some((i, group)) => (i + 1, group),
            None => (self.tabs.len(), None),
        };
        self.tabs.insert(
            index,
            Tab {
                id,
                url: url.to_string(),
                title: title.to_string(),
                favicon: None,
                loading: false,
                can_go_back: false,
                can_go_forward: false,
                group,
                error: None,
                live: false,
            },
        );
        self.mru.push(id);
        if activate || self.active.is_none() {
            self.activate(id);
        }
        id
    }

    pub fn activate(&mut self, id: TabId) -> bool {
        if self.get(id).is_none() {
            return false;
        }
        self.active = Some(id);
        self.mru.retain(|t| *t != id);
        self.mru.insert(0, id);
        // Activating a tab inside a collapsed group expands it so the tab is visible.
        if let Some(g) = self.get(id).and_then(|t| t.group) {
            if let Some(group) = self.groups.iter_mut().find(|x| x.id == g) {
                group.collapsed = false;
            }
        }
        true
    }

    /// Closes a tab and returns the id of the newly active tab (if the active tab changed).
    pub fn close(&mut self, id: TabId) -> Option<TabId> {
        let index = self.index_of(id)?;
        self.tabs.remove(index);
        self.mru.retain(|t| *t != id);
        self.prune_groups();
        if self.active == Some(id) {
            self.active = None;
            if let Some(next) = self.mru.first().copied() {
                self.activate(next);
            }
            return self.active;
        }
        None
    }

    /// Moves a tab to `to` (index in the strip). Group membership follows the drop position:
    /// dropping between two tabs of the same group joins it, dropping elsewhere leaves it.
    pub fn move_tab(&mut self, id: TabId, to: usize) -> bool {
        let Some(from) = self.index_of(id) else { return false };
        let tab = self.tabs.remove(from);
        let to = to.min(self.tabs.len());
        self.tabs.insert(to, tab);
        let left = to.checked_sub(1).and_then(|i| self.tabs.get(i)).and_then(|t| t.group);
        let right = self.tabs.get(to + 1).and_then(|t| t.group);
        let own = self.tabs[to].group;
        self.tabs[to].group = match (left, right) {
            (Some(l), Some(r)) if l == r => Some(l),
            _ if own.is_some() && (left == own || right == own) => own,
            _ => None,
        };
        self.normalize_groups();
        true
    }

    pub fn create_group(&mut self, name: &str, tabs: &[TabId]) -> Option<GroupId> {
        let members: Vec<TabId> = tabs.iter().copied().filter(|t| self.get(*t).is_some()).collect();
        if members.is_empty() {
            return None;
        }
        let id = self.next_group;
        self.next_group += 1;
        let color = GROUP_COLORS[(id as usize - 1) % GROUP_COLORS.len()].to_string();
        let name = clean_name(name);
        self.groups.push(Group { id, name, color, collapsed: false });
        for t in members {
            self.assign(t, Some(id));
        }
        Some(id)
    }

    /// Puts a tab into a group (moving it next to the group's other tabs) or removes it from one.
    pub fn assign(&mut self, tab: TabId, group: Option<GroupId>) -> bool {
        if let Some(g) = group {
            if self.group(g).is_none() {
                return false;
            }
        }
        let Some(from) = self.index_of(tab) else { return false };
        if self.tabs[from].group == group {
            return true;
        }
        let old_group = self.tabs[from].group;
        let mut moved = self.tabs.remove(from);
        moved.group = group;
        // Joining: place after the group's last tab. Leaving: place just after the old group's span.
        let insert_at = group
            .or(old_group)
            .and_then(|g| self.tabs.iter().rposition(|t| t.group == Some(g)).map(|i| i + 1))
            .unwrap_or(from)
            .min(self.tabs.len());
        self.tabs.insert(insert_at, moved);
        self.prune_groups();
        true
    }

    pub fn rename_group(&mut self, id: GroupId, name: &str) -> bool {
        match self.groups.iter_mut().find(|g| g.id == id) {
            Some(g) => {
                g.name = clean_name(name);
                true
            }
            None => false,
        }
    }

    pub fn set_group_color(&mut self, id: GroupId, color: &str) -> bool {
        if !GROUP_COLORS.contains(&color) {
            return false;
        }
        match self.groups.iter_mut().find(|g| g.id == id) {
            Some(g) => {
                g.color = color.to_string();
                true
            }
            None => false,
        }
    }

    /// Collapses or expands a group. Collapsing the group that holds the active tab switches to
    /// the nearest tab outside the group, if there is one. Returns the new active tab if changed.
    pub fn set_collapsed(&mut self, id: GroupId, collapsed: bool) -> Option<TabId> {
        let group = self.groups.iter_mut().find(|g| g.id == id)?;
        group.collapsed = collapsed;
        if !collapsed {
            return None;
        }
        let active = self.active?;
        if self.get(active).and_then(|t| t.group) != Some(id) {
            return None;
        }
        let next = self.mru.iter().copied().find(|t| self.get(*t).is_some_and(|tab| tab.group != Some(id)))?;
        self.active = Some(next);
        self.mru.retain(|t| *t != next);
        self.mru.insert(0, next);
        Some(next)
    }

    /// Removes a group. With `close_tabs` the member tabs are returned for closing; otherwise
    /// they are simply ungrouped.
    pub fn remove_group(&mut self, id: GroupId, close_tabs: bool) -> Vec<TabId> {
        let members: Vec<TabId> = self.tabs.iter().filter(|t| t.group == Some(id)).map(|t| t.id).collect();
        for t in self.tabs.iter_mut().filter(|t| t.group == Some(id)) {
            t.group = None;
        }
        self.groups.retain(|g| g.id != id);
        if close_tabs {
            members
        } else {
            Vec::new()
        }
    }

    /// Next/previous tab in strip order, skipping tabs hidden in collapsed groups.
    pub fn neighbour(&self, forward: bool) -> Option<TabId> {
        let visible: Vec<TabId> = self
            .tabs
            .iter()
            .filter(|t| Some(t.id) == self.active || !t.group.and_then(|g| self.group(g)).is_some_and(|g| g.collapsed))
            .map(|t| t.id)
            .collect();
        let pos = visible.iter().position(|t| Some(*t) == self.active)?;
        let n = visible.len();
        let next = if forward { (pos + 1) % n } else { (pos + n - 1) % n };
        visible.get(next).copied()
    }

    pub fn nth(&self, index: usize) -> Option<TabId> {
        self.tabs.get(index).map(|t| t.id)
    }

    pub fn last(&self) -> Option<TabId> {
        self.tabs.last().map(|t| t.id)
    }

    fn prune_groups(&mut self) {
        let tabs = &self.tabs;
        self.groups.retain(|g| tabs.iter().any(|t| t.group == Some(g.id)));
    }

    /// Restores contiguity after arbitrary moves: each group's tabs are pulled together at the
    /// position of the group's first tab, preserving relative order.
    fn normalize_groups(&mut self) {
        let mut out: Vec<Tab> = Vec::with_capacity(self.tabs.len());
        let mut placed: Vec<GroupId> = Vec::new();
        let tabs = std::mem::take(&mut self.tabs);
        for t in &tabs {
            match t.group {
                None => out.push(t.clone()),
                Some(g) if !placed.contains(&g) => {
                    placed.push(g);
                    out.extend(tabs.iter().filter(|x| x.group == Some(g)).cloned());
                }
                Some(_) => {}
            }
        }
        self.tabs = out;
        self.prune_groups();
    }

    pub fn to_session(&self) -> Session {
        Session {
            tabs: self
                .tabs
                .iter()
                .map(|t| SessionTab { url: t.url.clone(), title: t.title.clone(), group: t.group })
                .collect(),
            groups: self
                .groups
                .iter()
                .map(|g| SessionGroup { id: g.id, name: g.name.clone(), color: g.color.clone(), collapsed: g.collapsed })
                .collect(),
            active_index: self.active.and_then(|a| self.index_of(a)),
        }
    }

    /// Rebuilds tabs and groups from a saved session. Group ids are remapped to fresh ids.
    pub fn restore(&mut self, session: &Session) {
        let mut map = std::collections::HashMap::new();
        for g in &session.groups {
            let id = self.next_group;
            self.next_group += 1;
            map.insert(g.id, id);
            let color = if GROUP_COLORS.contains(&g.color.as_str()) { g.color.clone() } else { GROUP_COLORS[0].to_string() };
            self.groups.push(Group { id, name: clean_name(&g.name), color, collapsed: g.collapsed });
        }
        let mut ids = Vec::new();
        for t in &session.tabs {
            let id = self.add(&t.url, &t.title, None, false);
            if let Some(tab) = self.get_mut(id) {
                tab.group = t.group.and_then(|g| map.get(&g).copied());
            }
            ids.push(id);
        }
        self.normalize_groups();
        if let Some(id) = session.active_index.and_then(|i| ids.get(i)).copied() {
            self.activate(id);
        }
    }
}

fn clean_name(name: &str) -> String {
    let n: String = name.trim().chars().take(40).collect();
    if n.is_empty() {
        "Group".into()
    } else {
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(m: &TabManager) -> Vec<TabId> {
        m.tabs().iter().map(|t| t.id).collect()
    }

    fn contiguous(m: &TabManager) -> bool {
        let mut seen: Vec<GroupId> = Vec::new();
        let mut prev: Option<GroupId> = None;
        for t in m.tabs() {
            if t.group != prev {
                if let Some(g) = t.group {
                    if seen.contains(&g) {
                        return false;
                    }
                    seen.push(g);
                }
            }
            prev = t.group;
        }
        true
    }

    #[test]
    fn add_activate_close_uses_mru() {
        let mut m = TabManager::new();
        let a = m.add("https://a", "A", None, true);
        let b = m.add("https://b", "B", None, true);
        let c = m.add("https://c", "C", None, true);
        m.activate(a);
        m.activate(c);
        assert_eq!(m.close(c), Some(a));
        assert_eq!(m.close(b), None);
        assert_eq!(m.active(), Some(a));
        assert_eq!(m.close(a), None);
        assert_eq!(m.active(), None);
    }

    #[test]
    fn opened_from_tab_inserts_after_and_inherits_group() {
        let mut m = TabManager::new();
        let a = m.add("a", "", None, true);
        let _b = m.add("b", "", None, true);
        let g = m.create_group("Work", &[a]).unwrap();
        let c = m.add("c", "", Some(a), false);
        assert_eq!(m.index_of(c), Some(1));
        assert_eq!(m.get(c).unwrap().group, Some(g));
    }

    #[test]
    fn groups_stay_contiguous() {
        let mut m = TabManager::new();
        let t: Vec<_> = (0..5).map(|i| m.add(&format!("t{i}"), "", None, true)).collect();
        let g = m.create_group("G", &[t[0], t[3]]).unwrap();
        assert!(contiguous(&m));
        assert_eq!(&ids(&m)[..2], &[t[0], t[3]]);
        // Moving a non-member between members joins the group.
        m.move_tab(t[4], 1);
        assert_eq!(m.get(t[4]).unwrap().group, Some(g));
        assert!(contiguous(&m));
        // Moving a member far away leaves the group.
        m.move_tab(t[0], 4);
        assert_eq!(m.get(t[0]).unwrap().group, None);
        assert!(contiguous(&m));
        // Ungrouping all members prunes the group.
        m.assign(t[3], None);
        m.assign(t[4], None);
        assert!(m.group(g).is_none());
        assert!(contiguous(&m));
    }

    #[test]
    fn collapse_moves_focus_out_of_group() {
        let mut m = TabManager::new();
        let a = m.add("a", "", None, true);
        let b = m.add("b", "", None, true);
        let g = m.create_group("G", &[b]).unwrap();
        m.activate(b);
        assert_eq!(m.set_collapsed(g, true), Some(a));
        assert_eq!(m.neighbour(true), Some(a), "collapsed tabs are skipped");
        m.activate(b);
        assert!(!m.group(g).unwrap().collapsed, "activating a member expands the group");
    }

    #[test]
    fn remove_group_variants() {
        let mut m = TabManager::new();
        let a = m.add("a", "", None, true);
        let b = m.add("b", "", None, true);
        let g = m.create_group("G", &[a, b]).unwrap();
        assert!(m.remove_group(g, false).is_empty());
        assert!(m.tabs().iter().all(|t| t.group.is_none()));
        let g2 = m.create_group("G2", &[a, b]).unwrap();
        assert_eq!(m.remove_group(g2, true), vec![a, b]);
    }

    #[test]
    fn session_round_trip() {
        let mut m = TabManager::new();
        let a = m.add("https://a/", "A", None, true);
        let b = m.add("https://b/", "B", None, true);
        m.create_group("Reading", &[b]);
        m.activate(a);
        let session = m.to_session();

        let mut restored = TabManager::new();
        restored.restore(&session);
        assert_eq!(restored.tabs().len(), 2);
        assert_eq!(restored.active(), restored.nth(0));
        let gid = restored.tabs()[1].group.unwrap();
        assert_eq!(restored.group(gid).unwrap().name, "Reading");
        assert_eq!(restored.to_session(), {
            let mut s = session.clone();
            s.groups[0].id = gid;
            s.tabs[1].group = Some(gid);
            s
        });
    }
}
