use iced::font::{Family, Weight};
use iced::futures::channel::mpsc::Sender;
use iced::Font;
use parakesh_common::AppEvent;

#[derive(Clone, Debug, Default)]
pub(crate) enum UiMainTab {
    #[default]
    RecLN,
    RecEC,
    SendLN,
    SendEC,
    Mints,
    Settings,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum WidgetMessage {
    ShowLongStringModeString,
    ShowLongStringModeQR,
}

/// UI Messages
#[derive(Debug, Clone)]
pub(crate) enum Message {
    SubscriptionSender(Sender<AppEvent>),
    // RefreshNoop,
    RefreshInfo,
    AppEvent(AppEvent),
    Tab(UiMainTab),
    AmountInput(String),
    InvoiceInput(String),
    TokenInput(String),
    AddMintInput(String),
    ReceiveLN(u64),
    ReceiveLNOK,
    ReceiveEC(String),
    ReceiveECOK,
    SendLN(String),
    SendLNOK,
    SendEC(u64),
    SendECOK,
    SelectMint(String),
    AddMint(String),
    WidgetMessage((String, WidgetMessage)),
    CopyToClipboard(String),
}

pub(crate) struct MyFonts {}

impl MyFonts {
    pub fn default() -> Font {
        Font::DEFAULT
    }

    pub fn bold() -> Font {
        let mut font = Font::DEFAULT;
        font.weight = Weight::Bold;
        font
    }

    pub fn mono() -> Font {
        let mut font = Font::DEFAULT;
        font.family = Family::Monospace;
        font
    }

    pub fn bold_if(is_bold: bool) -> Font {
        if is_bold {
            Self::bold()
        } else {
            Self::default()
        }
    }
}
