//! Browser keyboard shortcuts. Key presses inside any webview (Zuno's UI or a website) are
//! intercepted through WebView2's `AcceleratorKeyPressed` event and mapped here, so shortcuts
//! work regardless of which webview has focus.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Shortcut {
    NewTab,
    CloseTab,
    ReopenClosedTab,
    NextTab,
    PreviousTab,
    SelectTab1,
    SelectTab2,
    SelectTab3,
    SelectTab4,
    SelectTab5,
    SelectTab6,
    SelectTab7,
    SelectTab8,
    SelectLastTab,
    FocusAddress,
    QuickSwitcher,
    Reload,
    Back,
    Forward,
    Bookmark,
    History,
    Downloads,
    Bookmarks,
    Settings,
    ToggleBookmarksBar,
    Fullscreen,
    IncognitoWindow,
}

impl Shortcut {
    /// Whether holding the key down should repeat the action.
    pub fn repeats(self) -> bool {
        matches!(self, Shortcut::NextTab | Shortcut::PreviousTab)
    }
}

// Win32 virtual-key codes used below.
const VK_TAB: u32 = 0x09;
const VK_LEFT: u32 = 0x25;
const VK_RIGHT: u32 = 0x27;
const VK_F4: u32 = 0x73;
const VK_F5: u32 = 0x74;
const VK_F6: u32 = 0x75;
const VK_F11: u32 = 0x7A;
const VK_OEM_COMMA: u32 = 0xBC;
const VK_BROWSER_BACK: u32 = 0xA6;
const VK_BROWSER_FORWARD: u32 = 0xA7;
const VK_BROWSER_REFRESH: u32 = 0xA8;

#[derive(Debug, Clone, Copy, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

pub fn map_key(vk: u32, m: Modifiers) -> Option<Shortcut> {
    use Shortcut::*;
    let key = char::from_u32(vk).filter(|c| c.is_ascii_alphanumeric());
    match (m.ctrl, m.shift, m.alt) {
        (true, false, false) => match (vk, key) {
            (_, Some('T')) => Some(NewTab),
            (_, Some('W')) | (VK_F4, _) => Some(CloseTab),
            (VK_TAB, _) => Some(NextTab),
            (_, Some('L')) => Some(FocusAddress),
            (_, Some('K')) => Some(QuickSwitcher),
            (_, Some('R')) | (VK_F5, _) => Some(Reload),
            (_, Some('D')) => Some(Bookmark),
            (_, Some('H')) => Some(History),
            (_, Some('J')) => Some(Downloads),
            (VK_OEM_COMMA, _) => Some(Settings),
            (_, Some(c @ '1'..='8')) => Some(match c {
                '1' => SelectTab1,
                '2' => SelectTab2,
                '3' => SelectTab3,
                '4' => SelectTab4,
                '5' => SelectTab5,
                '6' => SelectTab6,
                '7' => SelectTab7,
                _ => SelectTab8,
            }),
            (_, Some('9')) => Some(SelectLastTab),
            _ => None,
        },
        (true, true, false) => match (vk, key) {
            (_, Some('T')) => Some(ReopenClosedTab),
            (VK_TAB, _) => Some(PreviousTab),
            (_, Some('R')) => Some(Reload),
            (_, Some('O')) => Some(Bookmarks),
            (_, Some('B')) => Some(ToggleBookmarksBar),
            (_, Some('N')) => Some(IncognitoWindow),
            _ => None,
        },
        (false, false, true) => match (vk, key) {
            (VK_LEFT, _) => Some(Back),
            (VK_RIGHT, _) => Some(Forward),
            (_, Some('D')) => Some(FocusAddress),
            _ => None,
        },
        (false, false, false) => match vk {
            VK_F5 | VK_BROWSER_REFRESH => Some(Reload),
            VK_F6 => Some(FocusAddress),
            VK_F11 => Some(Fullscreen),
            VK_BROWSER_BACK => Some(Back),
            VK_BROWSER_FORWARD => Some(Forward),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CTRL: Modifiers = Modifiers { ctrl: true, shift: false, alt: false };
    const CTRL_SHIFT: Modifiers = Modifiers { ctrl: true, shift: true, alt: false };
    const ALT: Modifiers = Modifiers { ctrl: false, shift: false, alt: true };

    #[test]
    fn mapping() {
        assert_eq!(map_key('T' as u32, CTRL), Some(Shortcut::NewTab));
        assert_eq!(map_key('T' as u32, CTRL_SHIFT), Some(Shortcut::ReopenClosedTab));
        assert_eq!(map_key('K' as u32, CTRL), Some(Shortcut::QuickSwitcher));
        assert_eq!(map_key(VK_TAB, CTRL_SHIFT), Some(Shortcut::PreviousTab));
        assert_eq!(map_key('N' as u32, CTRL_SHIFT), Some(Shortcut::IncognitoWindow));
        assert_eq!(map_key('3' as u32, CTRL), Some(Shortcut::SelectTab3));
        assert_eq!(map_key(VK_LEFT, ALT), Some(Shortcut::Back));
        assert_eq!(map_key(VK_F5, Modifiers::default()), Some(Shortcut::Reload));
        // Editing shortcuts are left to the focused page or input.
        assert_eq!(map_key('C' as u32, CTRL), None);
        assert_eq!(map_key('V' as u32, CTRL), None);
        assert_eq!(map_key('F' as u32, CTRL), None);
        assert_eq!(map_key('A' as u32, Modifiers::default()), None);
    }
}
