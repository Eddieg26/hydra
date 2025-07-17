use input::{
    ButtonState, Key, KeyCode, MouseButton, MouseScrollDelta, MouseScrollUnit, NativeKey,
    NativeKeyCode, TouchForce, TouchPhase,
};
use winit::event::ElementState;

pub fn physical_key(key: winit::keyboard::PhysicalKey) -> KeyCode {
    match key {
        winit::keyboard::PhysicalKey::Unidentified(code) => KeyCode::Native(native_key_code(code)),
        winit::keyboard::PhysicalKey::Code(code) => match code {
            winit::keyboard::KeyCode::Digit0 => KeyCode::Digit0,
            winit::keyboard::KeyCode::Digit1 => KeyCode::Digit1,
            winit::keyboard::KeyCode::Digit2 => KeyCode::Digit2,
            winit::keyboard::KeyCode::Digit3 => KeyCode::Digit3,
            winit::keyboard::KeyCode::Digit4 => KeyCode::Digit4,
            winit::keyboard::KeyCode::Digit5 => KeyCode::Digit5,
            winit::keyboard::KeyCode::Digit6 => KeyCode::Digit6,
            winit::keyboard::KeyCode::Digit7 => KeyCode::Digit7,
            winit::keyboard::KeyCode::Digit8 => KeyCode::Digit8,
            winit::keyboard::KeyCode::Digit9 => KeyCode::Digit9,
            winit::keyboard::KeyCode::KeyA => KeyCode::KeyA,
            winit::keyboard::KeyCode::KeyB => KeyCode::KeyB,
            winit::keyboard::KeyCode::KeyC => KeyCode::KeyC,
            winit::keyboard::KeyCode::KeyD => KeyCode::KeyD,
            winit::keyboard::KeyCode::KeyE => KeyCode::KeyE,
            winit::keyboard::KeyCode::KeyF => KeyCode::KeyF,
            winit::keyboard::KeyCode::KeyG => KeyCode::KeyG,
            winit::keyboard::KeyCode::KeyH => KeyCode::KeyH,
            winit::keyboard::KeyCode::KeyI => KeyCode::KeyI,
            winit::keyboard::KeyCode::KeyJ => KeyCode::KeyJ,
            winit::keyboard::KeyCode::KeyK => KeyCode::KeyK,
            winit::keyboard::KeyCode::KeyL => KeyCode::KeyL,
            winit::keyboard::KeyCode::KeyM => KeyCode::KeyM,
            winit::keyboard::KeyCode::KeyN => KeyCode::KeyN,
            winit::keyboard::KeyCode::KeyO => KeyCode::KeyO,
            winit::keyboard::KeyCode::KeyP => KeyCode::KeyP,
            winit::keyboard::KeyCode::KeyQ => KeyCode::KeyQ,
            winit::keyboard::KeyCode::KeyR => KeyCode::KeyR,
            winit::keyboard::KeyCode::KeyS => KeyCode::KeyS,
            winit::keyboard::KeyCode::KeyT => KeyCode::KeyT,
            winit::keyboard::KeyCode::KeyU => KeyCode::KeyU,
            winit::keyboard::KeyCode::KeyV => KeyCode::KeyV,
            winit::keyboard::KeyCode::KeyW => KeyCode::KeyW,
            winit::keyboard::KeyCode::KeyX => KeyCode::KeyX,
            winit::keyboard::KeyCode::KeyY => KeyCode::KeyY,
            winit::keyboard::KeyCode::KeyZ => KeyCode::KeyZ,
            winit::keyboard::KeyCode::Escape => KeyCode::Escape,
            winit::keyboard::KeyCode::F1 => KeyCode::F1,
            winit::keyboard::KeyCode::F2 => KeyCode::F2,
            winit::keyboard::KeyCode::F3 => KeyCode::F3,
            winit::keyboard::KeyCode::F4 => KeyCode::F4,
            winit::keyboard::KeyCode::F5 => KeyCode::F5,
            winit::keyboard::KeyCode::F6 => KeyCode::F6,
            winit::keyboard::KeyCode::F7 => KeyCode::F7,
            winit::keyboard::KeyCode::F8 => KeyCode::F8,
            winit::keyboard::KeyCode::F9 => KeyCode::F9,
            winit::keyboard::KeyCode::F10 => KeyCode::F10,
            winit::keyboard::KeyCode::F11 => KeyCode::F11,
            winit::keyboard::KeyCode::F12 => KeyCode::F12,
            winit::keyboard::KeyCode::F13 => KeyCode::F13,
            winit::keyboard::KeyCode::F14 => KeyCode::F14,
            winit::keyboard::KeyCode::F15 => KeyCode::F15,
            winit::keyboard::KeyCode::F16 => KeyCode::F16,
            winit::keyboard::KeyCode::F17 => KeyCode::F17,
            winit::keyboard::KeyCode::F18 => KeyCode::F18,
            winit::keyboard::KeyCode::F19 => KeyCode::F19,
            winit::keyboard::KeyCode::F20 => KeyCode::F20,
            winit::keyboard::KeyCode::F21 => KeyCode::F21,
            winit::keyboard::KeyCode::F22 => KeyCode::F22,
            winit::keyboard::KeyCode::F23 => KeyCode::F23,
            winit::keyboard::KeyCode::F24 => KeyCode::F24,
            winit::keyboard::KeyCode::F25 => KeyCode::F25,
            winit::keyboard::KeyCode::F26 => KeyCode::F26,
            winit::keyboard::KeyCode::F27 => KeyCode::F27,
            winit::keyboard::KeyCode::F28 => KeyCode::F28,
            winit::keyboard::KeyCode::F29 => KeyCode::F29,
            winit::keyboard::KeyCode::F30 => KeyCode::F30,
            winit::keyboard::KeyCode::F31 => KeyCode::F31,
            winit::keyboard::KeyCode::F32 => KeyCode::F32,
            winit::keyboard::KeyCode::F33 => KeyCode::F33,
            winit::keyboard::KeyCode::F34 => KeyCode::F34,
            winit::keyboard::KeyCode::F35 => KeyCode::F35,
            winit::keyboard::KeyCode::Backquote => KeyCode::Backquote,
            winit::keyboard::KeyCode::Backslash => KeyCode::Backslash,
            winit::keyboard::KeyCode::BracketLeft => KeyCode::BracketLeft,
            winit::keyboard::KeyCode::BracketRight => KeyCode::BracketRight,
            winit::keyboard::KeyCode::Comma => KeyCode::Comma,
            winit::keyboard::KeyCode::Equal => KeyCode::Equal,
            winit::keyboard::KeyCode::IntlBackslash => KeyCode::IntlBackslash,
            winit::keyboard::KeyCode::IntlRo => KeyCode::IntlRo,
            winit::keyboard::KeyCode::IntlYen => KeyCode::IntlYen,
            winit::keyboard::KeyCode::Minus => KeyCode::Minus,
            winit::keyboard::KeyCode::Period => KeyCode::Period,
            winit::keyboard::KeyCode::Quote => KeyCode::Quote,
            winit::keyboard::KeyCode::Semicolon => KeyCode::Semicolon,
            winit::keyboard::KeyCode::Slash => KeyCode::Slash,
            winit::keyboard::KeyCode::AltLeft => KeyCode::AltLeft,
            winit::keyboard::KeyCode::AltRight => KeyCode::AltRight,
            winit::keyboard::KeyCode::Backspace => KeyCode::Backspace,
            winit::keyboard::KeyCode::CapsLock => KeyCode::CapsLock,
            winit::keyboard::KeyCode::ContextMenu => KeyCode::ContextMenu,
            winit::keyboard::KeyCode::ControlLeft => KeyCode::ControlLeft,
            winit::keyboard::KeyCode::ControlRight => KeyCode::ControlRight,
            winit::keyboard::KeyCode::Enter => KeyCode::Enter,
            winit::keyboard::KeyCode::SuperLeft => KeyCode::SuperLeft,
            winit::keyboard::KeyCode::SuperRight => KeyCode::SuperRight,
            winit::keyboard::KeyCode::ShiftLeft => KeyCode::ShiftLeft,
            winit::keyboard::KeyCode::ShiftRight => KeyCode::ShiftRight,
            winit::keyboard::KeyCode::Space => KeyCode::Space,
            winit::keyboard::KeyCode::Tab => KeyCode::Tab,
            winit::keyboard::KeyCode::Convert => KeyCode::Convert,
            winit::keyboard::KeyCode::KanaMode => KeyCode::KanaMode,
            winit::keyboard::KeyCode::Lang1 => KeyCode::Lang1,
            winit::keyboard::KeyCode::Lang2 => KeyCode::Lang2,
            winit::keyboard::KeyCode::Lang3 => KeyCode::Lang3,
            winit::keyboard::KeyCode::Lang4 => KeyCode::Lang4,
            winit::keyboard::KeyCode::Lang5 => KeyCode::Lang5,
            winit::keyboard::KeyCode::NonConvert => KeyCode::NonConvert,
            winit::keyboard::KeyCode::Delete => KeyCode::Delete,
            winit::keyboard::KeyCode::End => KeyCode::End,
            winit::keyboard::KeyCode::Help => KeyCode::Help,
            winit::keyboard::KeyCode::Home => KeyCode::Home,
            winit::keyboard::KeyCode::Insert => KeyCode::Insert,
            winit::keyboard::KeyCode::PageDown => KeyCode::PageDown,
            winit::keyboard::KeyCode::PageUp => KeyCode::PageUp,
            winit::keyboard::KeyCode::ArrowDown => KeyCode::ArrowDown,
            winit::keyboard::KeyCode::ArrowLeft => KeyCode::ArrowLeft,
            winit::keyboard::KeyCode::ArrowRight => KeyCode::ArrowRight,
            winit::keyboard::KeyCode::ArrowUp => KeyCode::ArrowUp,
            winit::keyboard::KeyCode::NumLock => KeyCode::NumLock,
            winit::keyboard::KeyCode::Numpad0 => KeyCode::Numpad0,
            winit::keyboard::KeyCode::Numpad1 => KeyCode::Numpad1,
            winit::keyboard::KeyCode::Numpad2 => KeyCode::Numpad2,
            winit::keyboard::KeyCode::Numpad3 => KeyCode::Numpad3,
            winit::keyboard::KeyCode::Numpad4 => KeyCode::Numpad4,
            winit::keyboard::KeyCode::Numpad5 => KeyCode::Numpad5,
            winit::keyboard::KeyCode::Numpad6 => KeyCode::Numpad6,
            winit::keyboard::KeyCode::Numpad7 => KeyCode::Numpad7,
            winit::keyboard::KeyCode::Numpad8 => KeyCode::Numpad8,
            winit::keyboard::KeyCode::Numpad9 => KeyCode::Numpad9,
            winit::keyboard::KeyCode::NumpadAdd => KeyCode::NumpadAdd,
            winit::keyboard::KeyCode::NumpadBackspace => KeyCode::NumpadBackspace,
            winit::keyboard::KeyCode::NumpadClear => KeyCode::NumpadClear,
            winit::keyboard::KeyCode::NumpadClearEntry => KeyCode::NumpadClearEntry,
            winit::keyboard::KeyCode::NumpadComma => KeyCode::NumpadComma,
            winit::keyboard::KeyCode::NumpadDecimal => KeyCode::NumpadDecimal,
            winit::keyboard::KeyCode::NumpadDivide => KeyCode::NumpadDivide,
            winit::keyboard::KeyCode::NumpadEnter => KeyCode::NumpadEnter,
            winit::keyboard::KeyCode::NumpadEqual => KeyCode::NumpadEqual,
            winit::keyboard::KeyCode::NumpadHash => KeyCode::NumpadHash,
            winit::keyboard::KeyCode::NumpadMemoryAdd => KeyCode::NumpadMemoryAdd,
            winit::keyboard::KeyCode::NumpadMemoryClear => KeyCode::NumpadMemoryClear,
            winit::keyboard::KeyCode::NumpadMemoryRecall => KeyCode::NumpadMemoryRecall,
            winit::keyboard::KeyCode::NumpadMemoryStore => KeyCode::NumpadMemoryStore,
            winit::keyboard::KeyCode::NumpadMemorySubtract => KeyCode::NumpadMemorySubtract,
            winit::keyboard::KeyCode::NumpadMultiply => KeyCode::NumpadMultiply,
            winit::keyboard::KeyCode::NumpadParenLeft => KeyCode::NumpadParenLeft,
            winit::keyboard::KeyCode::NumpadParenRight => KeyCode::NumpadParenRight,
            winit::keyboard::KeyCode::NumpadStar => KeyCode::NumpadStar,
            winit::keyboard::KeyCode::NumpadSubtract => KeyCode::NumpadSubtract,
            winit::keyboard::KeyCode::Fn => KeyCode::Fn,
            winit::keyboard::KeyCode::FnLock => KeyCode::FnLock,
            winit::keyboard::KeyCode::PrintScreen => KeyCode::PrintScreen,
            winit::keyboard::KeyCode::ScrollLock => KeyCode::ScrollLock,
            winit::keyboard::KeyCode::Pause => KeyCode::Pause,
            winit::keyboard::KeyCode::BrowserBack => KeyCode::BrowserBack,
            winit::keyboard::KeyCode::BrowserFavorites => KeyCode::BrowserFavorites,
            winit::keyboard::KeyCode::BrowserForward => KeyCode::BrowserForward,
            winit::keyboard::KeyCode::BrowserHome => KeyCode::BrowserHome,
            winit::keyboard::KeyCode::BrowserRefresh => KeyCode::BrowserRefresh,
            winit::keyboard::KeyCode::BrowserSearch => KeyCode::BrowserSearch,
            winit::keyboard::KeyCode::BrowserStop => KeyCode::BrowserStop,
            winit::keyboard::KeyCode::Eject => KeyCode::Eject,
            winit::keyboard::KeyCode::LaunchApp1 => KeyCode::LaunchApp1,
            winit::keyboard::KeyCode::LaunchApp2 => KeyCode::LaunchApp2,
            winit::keyboard::KeyCode::LaunchMail => KeyCode::LaunchMail,
            winit::keyboard::KeyCode::MediaPlayPause => KeyCode::MediaPlayPause,
            winit::keyboard::KeyCode::MediaSelect => KeyCode::MediaSelect,
            winit::keyboard::KeyCode::MediaStop => KeyCode::MediaStop,
            winit::keyboard::KeyCode::MediaTrackNext => KeyCode::MediaTrackNext,
            winit::keyboard::KeyCode::MediaTrackPrevious => KeyCode::MediaTrackPrevious,
            winit::keyboard::KeyCode::Power => KeyCode::Power,
            winit::keyboard::KeyCode::Sleep => KeyCode::Sleep,
            winit::keyboard::KeyCode::AudioVolumeDown => KeyCode::AudioVolumeDown,
            winit::keyboard::KeyCode::AudioVolumeMute => KeyCode::AudioVolumeMute,
            winit::keyboard::KeyCode::AudioVolumeUp => KeyCode::AudioVolumeUp,
            winit::keyboard::KeyCode::WakeUp => KeyCode::WakeUp,
            winit::keyboard::KeyCode::Meta => KeyCode::Meta,
            winit::keyboard::KeyCode::Hyper => KeyCode::Hyper,
            winit::keyboard::KeyCode::Turbo => KeyCode::Turbo,
            winit::keyboard::KeyCode::Abort => KeyCode::Abort,
            winit::keyboard::KeyCode::Resume => KeyCode::Resume,
            winit::keyboard::KeyCode::Suspend => KeyCode::Suspend,
            winit::keyboard::KeyCode::Again => KeyCode::Again,
            winit::keyboard::KeyCode::Copy => KeyCode::Copy,
            winit::keyboard::KeyCode::Cut => KeyCode::Cut,
            winit::keyboard::KeyCode::Find => KeyCode::Find,
            winit::keyboard::KeyCode::Open => KeyCode::Open,
            winit::keyboard::KeyCode::Paste => KeyCode::Paste,
            winit::keyboard::KeyCode::Props => KeyCode::Props,
            winit::keyboard::KeyCode::Select => KeyCode::Select,
            winit::keyboard::KeyCode::Undo => KeyCode::Undo,
            winit::keyboard::KeyCode::Hiragana => KeyCode::Hiragana,
            winit::keyboard::KeyCode::Katakana => KeyCode::Katakana,
            _ => KeyCode::Unknown,
        },
    }
}

pub fn logical_key(key: winit::keyboard::Key) -> Key {
    match key {
        winit::keyboard::Key::Character(c) => Key::Character(c),
        winit::keyboard::Key::Unidentified(native) => Key::Unidentified(native_key(native)),
        winit::keyboard::Key::Dead(c) => Key::Dead(c),
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Alt) => Key::Alt,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AltGraph) => Key::AltGraph,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::CapsLock) => Key::CapsLock,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Control) => Key::Control,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Fn) => Key::Fn,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::FnLock) => Key::FnLock,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::NumLock) => Key::NumLock,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ScrollLock) => Key::ScrollLock,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Shift) => Key::Shift,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Symbol) => Key::Symbol,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::SymbolLock) => Key::SymbolLock,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Meta) => Key::Meta,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Hyper) => Key::Hyper,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Super) => Key::Super,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter) => Key::Enter,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Tab) => Key::Tab,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Space) => Key::Space,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowDown) => Key::ArrowDown,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowLeft) => Key::ArrowLeft,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowRight) => Key::ArrowRight,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ArrowUp) => Key::ArrowUp,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::End) => Key::End,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Home) => Key::Home,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::PageDown) => Key::PageDown,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::PageUp) => Key::PageUp,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Backspace) => Key::Backspace,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Clear) => Key::Clear,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Copy) => Key::Copy,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::CrSel) => Key::CrSel,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Cut) => Key::Cut,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Delete) => Key::Delete,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::EraseEof) => Key::EraseEof,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ExSel) => Key::ExSel,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Insert) => Key::Insert,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Paste) => Key::Paste,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Redo) => Key::Redo,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Undo) => Key::Undo,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Accept) => Key::Accept,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Again) => Key::Again,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Attn) => Key::Attn,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Cancel) => Key::Cancel,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ContextMenu) => Key::ContextMenu,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape) => Key::Escape,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Execute) => Key::Execute,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Find) => Key::Find,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Help) => Key::Help,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Pause) => Key::Pause,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Play) => Key::Play,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Props) => Key::Props,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Select) => Key::Select,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ZoomIn) => Key::ZoomIn,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ZoomOut) => Key::ZoomOut,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::BrightnessDown) => {
            Key::BrightnessDown
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::BrightnessUp) => Key::BrightnessUp,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Eject) => Key::Eject,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LogOff) => Key::LogOff,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Power) => Key::Power,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::PowerOff) => Key::PowerOff,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::PrintScreen) => Key::PrintScreen,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Hibernate) => Key::Hibernate,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Standby) => Key::Standby,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::WakeUp) => Key::WakeUp,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AllCandidates) => Key::AllCandidates,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Alphanumeric) => Key::Alphanumeric,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::CodeInput) => Key::CodeInput,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Compose) => Key::Compose,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Convert) => Key::Convert,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::FinalMode) => Key::FinalMode,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::GroupFirst) => Key::GroupFirst,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::GroupLast) => Key::GroupLast,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::GroupNext) => Key::GroupNext,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::GroupPrevious) => Key::GroupPrevious,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ModeChange) => Key::ModeChange,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::NextCandidate) => Key::NextCandidate,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::NonConvert) => Key::NonConvert,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::PreviousCandidate) => {
            Key::PreviousCandidate
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Process) => Key::Process,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::SingleCandidate) => {
            Key::SingleCandidate
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::HangulMode) => Key::HangulMode,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::HanjaMode) => Key::HanjaMode,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::JunjaMode) => Key::JunjaMode,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Eisu) => Key::Eisu,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Hankaku) => Key::Hankaku,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Hiragana) => Key::Hiragana,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::HiraganaKatakana) => {
            Key::HiraganaKatakana
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::KanaMode) => Key::KanaMode,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::KanjiMode) => Key::KanjiMode,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Katakana) => Key::Katakana,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Romaji) => Key::Romaji,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Zenkaku) => Key::Zenkaku,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ZenkakuHankaku) => {
            Key::ZenkakuHankaku
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Soft1) => Key::Soft1,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Soft2) => Key::Soft2,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Soft3) => Key::Soft3,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Soft4) => Key::Soft4,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ChannelDown) => Key::ChannelDown,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ChannelUp) => Key::ChannelUp,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Close) => Key::Close,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MailForward) => Key::MailForward,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MailReply) => Key::MailReply,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MailSend) => Key::MailSend,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaClose) => Key::MediaClose,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaFastForward) => {
            Key::MediaFastForward
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaPause) => Key::MediaPause,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaPlay) => Key::MediaPlay,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaPlayPause) => {
            Key::MediaPlayPause
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaRecord) => Key::MediaRecord,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaRewind) => Key::MediaRewind,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaStop) => Key::MediaStop,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaTrackNext) => {
            Key::MediaTrackNext
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaTrackPrevious) => {
            Key::MediaTrackPrevious
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::New) => Key::New,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Open) => Key::Open,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Print) => Key::Print,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Save) => Key::Save,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::SpellCheck) => Key::SpellCheck,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Key11) => Key::Key11,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Key12) => Key::Key12,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AudioBalanceLeft) => {
            Key::AudioBalanceLeft
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AudioBalanceRight) => {
            Key::AudioBalanceRight
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AudioBassBoostDown) => {
            Key::AudioBassBoostDown
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AudioBassBoostToggle) => {
            Key::AudioBassBoostToggle
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AudioBassBoostUp) => {
            Key::AudioBassBoostUp
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AudioFaderFront) => {
            Key::AudioFaderFront
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AudioFaderRear) => {
            Key::AudioFaderRear
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AudioSurroundModeNext) => {
            Key::AudioSurroundModeNext
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AudioTrebleDown) => {
            Key::AudioTrebleDown
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AudioTrebleUp) => Key::AudioTrebleUp,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AudioVolumeDown) => {
            Key::AudioVolumeDown
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AudioVolumeUp) => Key::AudioVolumeUp,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AudioVolumeMute) => {
            Key::AudioVolumeMute
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MicrophoneToggle) => {
            Key::MicrophoneToggle
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MicrophoneVolumeDown) => {
            Key::MicrophoneVolumeDown
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MicrophoneVolumeUp) => {
            Key::MicrophoneVolumeUp
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MicrophoneVolumeMute) => {
            Key::MicrophoneVolumeMute
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::SpeechCorrectionList) => {
            Key::SpeechCorrectionList
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::SpeechInputToggle) => {
            Key::SpeechInputToggle
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LaunchApplication1) => {
            Key::LaunchApplication1
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LaunchApplication2) => {
            Key::LaunchApplication2
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LaunchCalendar) => {
            Key::LaunchCalendar
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LaunchContacts) => {
            Key::LaunchContacts
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LaunchMail) => Key::LaunchMail,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LaunchMediaPlayer) => {
            Key::LaunchMediaPlayer
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LaunchMusicPlayer) => {
            Key::LaunchMusicPlayer
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LaunchPhone) => Key::LaunchPhone,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LaunchScreenSaver) => {
            Key::LaunchScreenSaver
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LaunchSpreadsheet) => {
            Key::LaunchSpreadsheet
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LaunchWebBrowser) => {
            Key::LaunchWebBrowser
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LaunchWebCam) => Key::LaunchWebCam,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LaunchWordProcessor) => {
            Key::LaunchWordProcessor
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::BrowserBack) => Key::BrowserBack,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::BrowserFavorites) => {
            Key::BrowserFavorites
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::BrowserForward) => {
            Key::BrowserForward
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::BrowserHome) => Key::BrowserHome,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::BrowserRefresh) => {
            Key::BrowserRefresh
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::BrowserSearch) => Key::BrowserSearch,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::BrowserStop) => Key::BrowserStop,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AppSwitch) => Key::AppSwitch,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Call) => Key::Call,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Camera) => Key::Camera,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::CameraFocus) => Key::CameraFocus,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::EndCall) => Key::EndCall,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::GoBack) => Key::GoBack,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::GoHome) => Key::GoHome,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::HeadsetHook) => Key::HeadsetHook,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LastNumberRedial) => {
            Key::LastNumberRedial
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Notification) => Key::Notification,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MannerMode) => Key::MannerMode,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::VoiceDial) => Key::VoiceDial,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TV) => Key::TV,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TV3DMode) => Key::TV3DMode,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVAntennaCable) => {
            Key::TVAntennaCable
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVAudioDescription) => {
            Key::TVAudioDescription
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVAudioDescriptionMixDown) => {
            Key::TVAudioDescriptionMixDown
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVAudioDescriptionMixUp) => {
            Key::TVAudioDescriptionMixUp
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVContentsMenu) => {
            Key::TVContentsMenu
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVDataService) => Key::TVDataService,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVInput) => Key::TVInput,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVInputComponent1) => {
            Key::TVInputComponent1
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVInputComponent2) => {
            Key::TVInputComponent2
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVInputComposite1) => {
            Key::TVInputComposite1
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVInputComposite2) => {
            Key::TVInputComposite2
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVInputHDMI1) => Key::TVInputHDMI1,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVInputHDMI2) => Key::TVInputHDMI2,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVInputHDMI3) => Key::TVInputHDMI3,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVInputHDMI4) => Key::TVInputHDMI4,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVInputVGA1) => Key::TVInputVGA1,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVMediaContext) => {
            Key::TVMediaContext
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVNetwork) => Key::TVNetwork,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVNumberEntry) => Key::TVNumberEntry,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVPower) => Key::TVPower,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVRadioService) => {
            Key::TVRadioService
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVSatellite) => Key::TVSatellite,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVSatelliteBS) => Key::TVSatelliteBS,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVSatelliteCS) => Key::TVSatelliteCS,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVSatelliteToggle) => {
            Key::TVSatelliteToggle
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVTerrestrialAnalog) => {
            Key::TVTerrestrialAnalog
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVTerrestrialDigital) => {
            Key::TVTerrestrialDigital
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::TVTimer) => Key::TVTimer,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AVRInput) => Key::AVRInput,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::AVRPower) => Key::AVRPower,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ColorF0Red) => Key::ColorF0Red,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ColorF1Green) => Key::ColorF1Green,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ColorF2Yellow) => Key::ColorF2Yellow,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ColorF3Blue) => Key::ColorF3Blue,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ColorF4Grey) => Key::ColorF4Grey,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ColorF5Brown) => Key::ColorF5Brown,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ClosedCaptionToggle) => {
            Key::ClosedCaptionToggle
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Dimmer) => Key::Dimmer,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::DisplaySwap) => Key::DisplaySwap,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::DVR) => Key::DVR,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Exit) => Key::Exit,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::FavoriteClear0) => {
            Key::FavoriteClear0
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::FavoriteClear1) => {
            Key::FavoriteClear1
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::FavoriteClear2) => {
            Key::FavoriteClear2
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::FavoriteClear3) => {
            Key::FavoriteClear3
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::FavoriteRecall0) => {
            Key::FavoriteRecall0
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::FavoriteRecall1) => {
            Key::FavoriteRecall1
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::FavoriteRecall2) => {
            Key::FavoriteRecall2
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::FavoriteRecall3) => {
            Key::FavoriteRecall3
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::FavoriteStore0) => {
            Key::FavoriteStore0
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::FavoriteStore1) => {
            Key::FavoriteStore1
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::FavoriteStore2) => {
            Key::FavoriteStore2
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::FavoriteStore3) => {
            Key::FavoriteStore3
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Guide) => Key::Guide,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::GuideNextDay) => Key::GuideNextDay,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::GuidePreviousDay) => {
            Key::GuidePreviousDay
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Info) => Key::Info,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::InstantReplay) => Key::InstantReplay,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Link) => Key::Link,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ListProgram) => Key::ListProgram,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::LiveContent) => Key::LiveContent,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Lock) => Key::Lock,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaApps) => Key::MediaApps,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaAudioTrack) => {
            Key::MediaAudioTrack
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaLast) => Key::MediaLast,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaSkipBackward) => {
            Key::MediaSkipBackward
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaSkipForward) => {
            Key::MediaSkipForward
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaStepBackward) => {
            Key::MediaStepBackward
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaStepForward) => {
            Key::MediaStepForward
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::MediaTopMenu) => Key::MediaTopMenu,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::NavigateIn) => Key::NavigateIn,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::NavigateNext) => Key::NavigateNext,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::NavigateOut) => Key::NavigateOut,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::NavigatePrevious) => {
            Key::NavigatePrevious
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::NextFavoriteChannel) => {
            Key::NextFavoriteChannel
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::NextUserProfile) => {
            Key::NextUserProfile
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::OnDemand) => Key::OnDemand,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Pairing) => Key::Pairing,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::PinPDown) => Key::PinPDown,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::PinPMove) => Key::PinPMove,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::PinPToggle) => Key::PinPToggle,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::PinPUp) => Key::PinPUp,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::PlaySpeedDown) => Key::PlaySpeedDown,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::PlaySpeedReset) => {
            Key::PlaySpeedReset
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::PlaySpeedUp) => Key::PlaySpeedUp,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::RandomToggle) => Key::RandomToggle,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::RcLowBattery) => Key::RcLowBattery,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::RecordSpeedNext) => {
            Key::RecordSpeedNext
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::RfBypass) => Key::RfBypass,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ScanChannelsToggle) => {
            Key::ScanChannelsToggle
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ScreenModeNext) => {
            Key::ScreenModeNext
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Settings) => Key::Settings,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::SplitScreenToggle) => {
            Key::SplitScreenToggle
        }
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::STBInput) => Key::STBInput,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::STBPower) => Key::STBPower,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Subtitle) => Key::Subtitle,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Teletext) => Key::Teletext,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::VideoModeNext) => Key::VideoModeNext,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::Wink) => Key::Wink,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::ZoomToggle) => Key::ZoomToggle,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F1) => Key::F1,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F2) => Key::F2,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F3) => Key::F3,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F4) => Key::F4,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F5) => Key::F5,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F6) => Key::F6,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F7) => Key::F7,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F8) => Key::F8,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F9) => Key::F9,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F10) => Key::F10,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F11) => Key::F11,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F12) => Key::F12,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F13) => Key::F13,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F14) => Key::F14,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F15) => Key::F15,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F16) => Key::F16,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F17) => Key::F17,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F18) => Key::F18,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F19) => Key::F19,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F20) => Key::F20,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F21) => Key::F21,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F22) => Key::F22,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F23) => Key::F23,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F24) => Key::F24,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F25) => Key::F25,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F26) => Key::F26,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F27) => Key::F27,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F28) => Key::F28,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F29) => Key::F29,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F30) => Key::F30,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F31) => Key::F31,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F32) => Key::F32,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F33) => Key::F33,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F34) => Key::F34,
        winit::keyboard::Key::Named(winit::keyboard::NamedKey::F35) => Key::F35,
        _ => todo!(),
    }
}

pub fn native_key(key: winit::keyboard::NativeKey) -> NativeKey {
    match key {
        winit::keyboard::NativeKey::Unidentified => NativeKey::Unidentified,
        winit::keyboard::NativeKey::Android(v) => NativeKey::Android(v),
        winit::keyboard::NativeKey::MacOS(v) => NativeKey::MacOS(v),
        winit::keyboard::NativeKey::Windows(v) => NativeKey::Windows(v),
        winit::keyboard::NativeKey::Xkb(v) => NativeKey::Xkb(v),
        winit::keyboard::NativeKey::Web(v) => NativeKey::Web(v),
    }
}

pub fn native_key_code(key: winit::keyboard::NativeKeyCode) -> NativeKeyCode {
    match key {
        winit::keyboard::NativeKeyCode::Unidentified => NativeKeyCode::Unidentified,
        winit::keyboard::NativeKeyCode::Android(v) => NativeKeyCode::Android(v),
        winit::keyboard::NativeKeyCode::MacOS(v) => NativeKeyCode::MacOS(v),
        winit::keyboard::NativeKeyCode::Windows(v) => NativeKeyCode::Windows(v),
        winit::keyboard::NativeKeyCode::Xkb(v) => NativeKeyCode::Xkb(v),
    }
}

pub fn button_state(state: ElementState) -> ButtonState {
    match state {
        ElementState::Pressed => ButtonState::Pressed,
        ElementState::Released => ButtonState::Released,
    }
}

pub fn mouse_delta(delta: winit::event::MouseScrollDelta) -> MouseScrollDelta {
    match delta {
        winit::event::MouseScrollDelta::LineDelta(x, y) => MouseScrollDelta {
            unit: MouseScrollUnit::Line,
            x,
            y,
        },
        winit::event::MouseScrollDelta::PixelDelta(pos) => MouseScrollDelta {
            unit: MouseScrollUnit::Pixel,
            x: pos.x as f32,
            y: pos.y as f32,
        },
    }
}

pub fn mouse_button(button: winit::event::MouseButton) -> MouseButton {
    match button {
        winit::event::MouseButton::Left => MouseButton::Left,
        winit::event::MouseButton::Right => MouseButton::Right,
        winit::event::MouseButton::Middle => MouseButton::Middle,
        winit::event::MouseButton::Other(id) => MouseButton::Other(id),
        winit::event::MouseButton::Back => MouseButton::Back,
        winit::event::MouseButton::Forward => MouseButton::Forward,
    }
}

pub fn touch_phase(phase: winit::event::TouchPhase) -> TouchPhase {
    match phase {
        winit::event::TouchPhase::Started => TouchPhase::Started,
        winit::event::TouchPhase::Moved => TouchPhase::Moved,
        winit::event::TouchPhase::Ended => TouchPhase::Ended,
        winit::event::TouchPhase::Cancelled => TouchPhase::Cancelled,
    }
}

pub fn touch_force(force: winit::event::Force) -> TouchForce {
    match force {
        winit::event::Force::Calibrated {
            force,
            max_possible_force,
            altitude_angle,
        } => TouchForce::Calibrated {
            force,
            max_possible_force,
            altitude_angle,
        },
        winit::event::Force::Normalized(v) => TouchForce::Normalized(v),
    }
}
