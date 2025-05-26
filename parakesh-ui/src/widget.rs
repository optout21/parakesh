use crate::common::{Message, MyFonts, WidgetMessage};
use iced::widget::qr_code::{Data, ErrorCorrection};
use iced::widget::{button, qr_code, row, text, Column, Row};
use iced::Element;

#[derive(Clone, Debug, Default)]
enum ShowMode {
    #[default]
    String,
    QRCode,
}

/// A 'widget' to show a long string, string or QR code representation
/// Not really a widget in the iced sense
#[derive(Default)]
struct ShowLongStringWidget {
    name: String,
    entity_name: String,
    data_string: Option<String>,
    data_bin: Option<Vec<u8>>,
    data_qr: Option<Data>,
    show_mode: ShowMode,
}

/// TODO: use iced 13.1 advanced::widget::text for proper wrapping
fn wrap_text(text: &String) -> String {
    let tl = text.len();
    let mut out = String::with_capacity(tl * 2);
    let mut pos = 0;
    let cs = 50;
    loop {
        let end = std::cmp::min(pos + cs, tl);
        out += text.get(pos..end).unwrap();
        if pos + cs >= tl {
            break;
        }
        pos += cs;
        out += " ";
    }
    out
}

/// Shorten a text, by omitting the tail, showing an ellipse and the last few letters
fn short_string(s: &str, len: usize) -> String {
    if len < 8 {
        s.to_owned()
    } else {
        let ll = s.len();
        format!("{}...{}", &s[0..len - 6], &s[ll - 3..ll])
    }
}

impl ShowLongStringWidget {
    fn new(name: String, entity_name: String) -> Self {
        Self {
            name,
            entity_name,
            data_string: None,
            data_bin: None,
            data_qr: None,
            show_mode: ShowMode::String,
        }
    }

    /// Optionally bin_data for QR code can be set separately,
    /// by default (if it's None), the string is used.
    fn set_data(&mut self, data_string: Option<String>, data_bin: Option<Vec<u8>>) {
        if let Some(data_string) = data_string {
            let mut bin = match data_bin {
                Some(data) => data,
                None => data_string.as_bytes().to_vec(),
            };
            self.data_bin = Some(bin.clone());
            match Data::with_error_correction(bin, ErrorCorrection::Low) {
                Ok(qr) => {
                    self.data_qr = Some(qr);
                }
                Err(err) => {
                    println!("QR error: {}", err);
                    // Put the error in the QR code
                    bin = format!("Error: {}", err).as_bytes().to_vec();
                    self.data_bin = Some(bin.clone());
                    self.data_qr = Data::new(bin).ok();
                }
            }
            self.data_string = Some(data_string);
        } else {
            self.data_string = None;
            self.data_bin = None;
            self.data_qr = None;
        }
    }

    fn view(&self) -> Element<Message> {
        let mut header_arr: Vec<Element<Message>> = vec![button("Copy")
            .on_press(Message::CopyToClipboard(
                self.data_string.as_ref().unwrap_or(&"".to_owned()).clone(),
            ))
            .into()];
        let mut contents_arr: Vec<Element<Message>> = Vec::new();

        match self.show_mode {
            ShowMode::String => {
                header_arr.push(
                    button("Show QR")
                        .on_press(Message::WidgetMessage((
                            self.name.clone(),
                            WidgetMessage::ShowLongStringModeQR,
                        )))
                        .into(),
                );
                let textc = wrap_text(&self.data_string.as_ref().unwrap_or(&"(empty)".to_owned()));
                contents_arr
                    .push(row![text(textc).font(MyFonts::mono()).size(15).height(400)].into());
            }
            ShowMode::QRCode => {
                header_arr.push(
                    button("Show String")
                        .on_press(Message::WidgetMessage((
                            self.name.clone(),
                            WidgetMessage::ShowLongStringModeString,
                        )))
                        .into(),
                );
                match &self.data_qr {
                    Some(data) => contents_arr.push(row![qr_code(data)].into()),
                    None => contents_arr.push(row![text("(empty QR)")].into()),
                }
            }
        }

        let text_short = match &self.data_string {
            Some(tl) => short_string(tl, 50),
            None => "(empty)".to_owned(),
        };

        let mut col_arr: Vec<Element<Message>> = vec![
            row![
                text(format!("{}: ", self.entity_name)).size(15),
                text(text_short).font(MyFonts::mono()).size(15),
            ]
            .into(),
            Row::with_children(header_arr).spacing(10).into(),
        ];
        col_arr.extend(contents_arr);
        Column::with_children(col_arr)
            .spacing(10)
            .height(600)
            .into()
    }

    fn update(&mut self, name: &str, wmsg: &WidgetMessage) {
        if *name == self.name {
            // for me
            match wmsg {
                WidgetMessage::ShowLongStringModeQR => self.show_mode = ShowMode::QRCode,
                WidgetMessage::ShowLongStringModeString => self.show_mode = ShowMode::String,
            }
        }
    }
}

#[derive(Default)]
pub(crate) struct ShowTokenWidget {
    base: ShowLongStringWidget,
}

impl ShowTokenWidget {
    pub(crate) fn new(name: String) -> Self {
        Self {
            base: ShowLongStringWidget::new(name, "Ecash token".to_owned()),
        }
    }

    pub(crate) fn set_data(&mut self, data_string: Option<String>, data_bin: Option<Vec<u8>>) {
        self.base.set_data(data_string, data_bin);
    }

    pub(crate) fn view(&self) -> Element<Message> {
        self.base.view()
    }

    pub(crate) fn update(&mut self, name: &str, wmsg: &WidgetMessage) {
        self.base.update(name, wmsg)
    }
}

#[derive(Default)]
pub(crate) struct ShowInvoiceWidget {
    base: ShowLongStringWidget,
}

impl ShowInvoiceWidget {
    pub(crate) fn new(name: String) -> Self {
        Self {
            base: ShowLongStringWidget::new(name, "LN invoice".to_owned()),
        }
    }

    pub(crate) fn set_data(&mut self, data_string: Option<String>, data_bin: Option<Vec<u8>>) {
        self.base.set_data(data_string, data_bin);
    }

    pub(crate) fn view(&self) -> Element<Message> {
        self.base.view()
    }

    pub(crate) fn update(&mut self, name: &str, wmsg: &WidgetMessage) {
        self.base.update(name, wmsg)
    }
}
