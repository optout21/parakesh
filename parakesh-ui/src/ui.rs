use crate::common::{Message, MyFonts, UiMainTab};
use crate::widget::{ShowInvoiceWidget, ShowTokenWidget};
use parakesh_common::pk_app::{BalanceInfo, MintInfo, MintsSummary, WalletInfo};
use parakesh_common::{AppEvent, PKAppAsync};

use iced::clipboard;
use iced::futures::channel::mpsc;
use iced::futures::{SinkExt, StreamExt};
use iced::widget::{button, column, mouse_area, row, scrollable, text, text_input, Column};
use iced::{Element, Renderer, Task, Theme};

#[derive(Default)]
enum AddMintState {
    #[default]
    NotRequested,
    Requested,
    Completed(Result<(), String>),
}

#[derive(Default)]
enum RecLNState {
    #[default]
    NotRequested,
    Requested(u64),
    InvoiceReady(String),
    Completed(Result<u64, String>),
}

#[derive(Default)]
enum RecECState {
    #[default]
    NotRequested,
    Requested,
    Completed(Result<u64, String>),
}

#[derive(Default)]
enum SendLNState {
    #[default]
    NotRequested,
    Requested,
    Completed(Result<u64, String>),
}

#[derive(Default)]
enum SendECState {
    #[default]
    NotRequested,
    Requested,
    Completed(Result<(u64, String), String>),
}

pub(crate) struct IcedApp {
    app: PKAppAsync,

    wallet_info: Option<WalletInfo>,
    balance: Option<BalanceInfo>,
    mints_info: Vec<MintInfo>,
    main_tab: UiMainTab,

    amount_input: String,
    invoice_input: String,
    token_input: String,
    add_mint_input: String,

    add_mint_state: AddMintState,
    rec_ln_state: RecLNState,
    rec_ec_state: RecECState,
    send_ln_state: SendLNState,
    send_ec_state: SendECState,
    show_invoice_widget: ShowInvoiceWidget,
    show_token_widget: ShowTokenWidget,
}

impl IcedApp {
    fn refresh_info(&mut self) {
        let _res = self.app.get_balance_and_wallet_info();
        let _res = self.app.get_mints_info();
    }

    fn amount_input(&self) -> Element<Message> {
        row![
            text("Amount: ").size(20),
            text_input("0", &self.amount_input)
                .on_input(Message::AmountInput)
                .size(20)
                .width(100),
            text("sats").size(20),
        ]
        .spacing(5)
        .into()
    }

    fn invoice_input(&self) -> Element<Message> {
        row![
            text("LN invoice: ").size(20),
            text_input("0", &self.invoice_input)
                .on_input(Message::InvoiceInput)
                .size(20)
                .width(400),
        ]
        .spacing(5)
        .into()
    }

    fn token_input(&self) -> Element<Message> {
        row![
            text("Ecash token: ").size(20),
            text_input("", &self.token_input)
                .on_input(Message::TokenInput)
                .size(20)
                .width(400),
        ]
        .spacing(5)
        .into()
    }

    fn view_rec_ln(&self) -> Element<Message> {
        let contents: Element<Message> = match &self.rec_ln_state {
            RecLNState::NotRequested => {
                // No receive in progress
                column![
                    self.amount_input(),
                    row![button("Receive").on_press(Message::ReceiveLN(
                        self.amount_input.parse::<u64>().unwrap_or_default()
                    )),],
                ]
                .spacing(10)
            }
            RecLNState::Requested(amount) => column![
                row![text(format!("Invoice requested for {} sats ...", amount)).size(20)],
                button("(Cancel)").on_press(Message::ReceiveLNOK),
            ]
            .spacing(10),
            RecLNState::InvoiceReady(_invoice) => column![
                row![text("Pay the invoice").size(20)],
                button("(Cancel)").on_press(Message::ReceiveLNOK),
                scrollable(self.show_invoice_widget.view()),
            ]
            .spacing(10),
            RecLNState::Completed(Err(err)) => column![
                row![text(format!("ERROR: {}", err)).size(20)],
                button("OK").on_press(Message::ReceiveLNOK),
            ]
            .spacing(10),
            RecLNState::Completed(Ok(amount)) => column![
                row![text(format!("Received {} sats", amount)).size(20)],
                button("OK").on_press(Message::ReceiveLNOK),
            ]
            .spacing(10),
        }
        .into();
        column![row![text("Receive Lightning").size(20)], contents,]
            .spacing(10)
            .into()
    }

    fn view_rec_ec(&self) -> Element<Message> {
        let contents: Element<Message> = match &self.rec_ec_state {
            RecECState::NotRequested => {
                // No receive in progress
                column![
                    self.token_input(),
                    row![button("Receive").on_press(Message::ReceiveEC(self.token_input.clone())),],
                ]
                .spacing(10)
            }
            RecECState::Requested => column![
                row![text(format!("Receive in progress...")).size(20)],
                button("(Cancel)").on_press(Message::ReceiveECOK),
            ]
            .spacing(10),
            RecECState::Completed(Err(err)) => column![
                row![text(format!("ERROR: {}", err)).size(20)],
                button("OK").on_press(Message::ReceiveECOK),
            ]
            .spacing(10),
            RecECState::Completed(Ok(amount)) => column![
                row![text(format!("Received {} sats", amount)).size(20)],
                button("OK").on_press(Message::ReceiveECOK),
            ]
            .spacing(10),
        }
        .into();
        column![row![text("Receive Ecash").size(20)], contents,]
            .spacing(10)
            .into()
    }

    fn view_send_ln(&self) -> Element<Message> {
        let contents: Element<Message> = match &self.send_ln_state {
            &SendLNState::NotRequested => {
                // No send in progress
                column![
                    self.invoice_input(),
                    row![button("Send (pay the invoice)")
                        .on_press(Message::SendLN(self.invoice_input.clone()))],
                ]
                .spacing(10)
            }
            SendLNState::Requested => column![
                row![text(format!("Send in progress...")).size(20)],
                button("(Cancel)").on_press(Message::SendLNOK),
            ]
            .spacing(10),
            SendLNState::Completed(Err(err)) => column![
                row![text(format!("ERROR: {}", err)).size(20)],
                button("OK").on_press(Message::SendLNOK),
            ]
            .spacing(10),
            SendLNState::Completed(Ok(amount)) => column![
                row![text(format!("Sent {} sats, paid the invoice", amount)).size(20)],
                button("OK").on_press(Message::SendLNOK),
            ]
            .spacing(10),
        }
        .into();
        column![row![text("Send Lightning").size(20)], contents,]
            .spacing(10)
            .into()
    }

    fn view_send_ec(&self) -> Element<Message> {
        let contents: Element<Message> = match &self.send_ec_state {
            SendECState::NotRequested => {
                // Prepare for send
                column![
                    self.amount_input(),
                    row![button("Send Ecash").on_press(Message::SendEC(
                        self.amount_input.parse::<u64>().unwrap_or_default()
                    )),]
                ]
                .spacing(10)
            }
            SendECState::Requested => column![
                row![text("Request in progress...").size(20)],
                row![button("OK").on_press(Message::SendECOK),],
            ]
            .spacing(10),
            SendECState::Completed(Ok(_res)) => column![
                row![button("OK").on_press(Message::SendECOK),],
                scrollable(self.show_token_widget.view()),
            ]
            .spacing(10),
            SendECState::Completed(Err(err)) => column![
                row![text(format!("ERROR: {}", err)).size(20)],
                row![button("OK").on_press(Message::SendECOK),],
            ]
            .spacing(10),
        }
        .into();
        column![row![text("Send Ecash").size(20)], contents,]
            .spacing(10)
            .into()
    }

    fn view_mints(&self) -> Element<Message> {
        let selected_mint = self
            .wallet_info
            .as_ref()
            .map(|wi| wi.selected_mint_url.to_string())
            .unwrap_or("?".to_owned());

        let mints_ui: Column<'_, Message, Theme, Renderer> =
            Column::with_children(self.mints_info.iter().map(|mi| {
                mouse_area(row![
                    text(mi.url.to_string())
                        .font(MyFonts::bold_if(mi.url.to_string() == selected_mint))
                        .size(15)
                        .width(300),
                    text(format!("  {}", mi.balance))
                        .font(MyFonts::bold_if(mi.url.to_string() == selected_mint))
                        .size(15),
                ])
                .on_press(Message::SelectMint(mi.url.to_string()))
                .into()
            }));
        column![
            row![text("Mints:").size(20)],
            row![text("Selected: ").size(15), text(selected_mint).size(15),],
            row![text("List (click to select)").size(15),],
            mints_ui,
            row![
                button("Add Mint:").on_press(Message::AddMint(self.add_mint_input.clone())),
                text_input("(mint url)", &self.add_mint_input)
                    .on_input(Message::AddMintInput)
                    .size(20)
                    .width(200),
            ]
            .spacing(10),
            row![text(match &self.add_mint_state {
                AddMintState::NotRequested => "-".to_owned(),
                AddMintState::Requested => "Add in progress...".to_owned(),
                AddMintState::Completed(Ok(_)) => "Mint added".to_owned(),
                AddMintState::Completed(Err(e)) => format!("Error adding mint, {}", e),
            })
            .size(15),]
            .spacing(10),
        ]
        .spacing(10)
        .into()
    }

    fn view_settings(&self) -> Element<Message> {
        column![row![text("Settings").size(20)], row![text("TODO").size(20)],].into()
    }

    fn view_header(&self) -> Element<Message> {
        let wallet_info = match &self.wallet_info {
            None => "?".to_owned(),
            Some(wi) => {
                if !wi.is_inititalized {
                    "Not initialized".to_owned()
                } else {
                    let mut buf = "OK  ".to_owned();
                    buf.push_str("Current mint: ");
                    buf.push_str(wi.selected_mint_url.as_str());
                    buf.push_str("  ");
                    buf.push_str(
                        match &wi.mints_summary {
                            MintsSummary::None => "No mints".to_owned(),
                            MintsSummary::Single(_mint) => "".to_owned(),
                            MintsSummary::Multiple(cnt) => format!("({} total)", cnt),
                        }
                        .as_str(),
                    );
                    buf
                }
            }
        };
        let balance = match &self.balance {
            None => "?".to_owned(),
            Some(bi) => bi.0.to_string(),
        };

        column![
            row![
                text("Balance: ").size(20),
                text(balance).size(20),
                text(" sats").size(20),
            ],
            row![text("Wallet: ").size(15), text(wallet_info).size(15),],
        ]
        .into()
    }

    fn view_main(&self) -> Element<Message> {
        let header = self.view_header();

        let tab_header: Element<Message> = row![
            button("Mints").on_press(Message::Tab(UiMainTab::Mints)),
            button("Receive LN").on_press(Message::Tab(UiMainTab::RecLN)),
            button("Receive EC").on_press(Message::Tab(UiMainTab::RecEC)),
            button("Send LN").on_press(Message::Tab(UiMainTab::SendLN)),
            button("Send EC").on_press(Message::Tab(UiMainTab::SendEC)),
            button("Settings").on_press(Message::Tab(UiMainTab::Settings)),
            text("|").size(20),
            button("(Refresh)").on_press(Message::RefreshInfo),
        ]
        .spacing(10)
        .into();

        let tab_view = match self.main_tab {
            UiMainTab::RecLN => self.view_rec_ln(),
            UiMainTab::RecEC => self.view_rec_ec(),
            UiMainTab::SendLN => self.view_send_ln(),
            UiMainTab::SendEC => self.view_send_ec(),
            UiMainTab::Mints => self.view_mints(),
            UiMainTab::Settings => self.view_settings(),
        };

        column![header, tab_header, tab_view,]
            .spacing(10)
            .padding(10)
            .into()
    }
}

impl IcedApp {
    pub fn new(backend: PKAppAsync) -> Self {
        IcedApp {
            app: backend,
            wallet_info: None,
            balance: None,
            mints_info: Vec::new(),
            main_tab: UiMainTab::RecLN,
            amount_input: "0".to_owned(),
            invoice_input: "".to_owned(),
            token_input: "".to_owned(),
            add_mint_input: "".to_owned(),

            add_mint_state: AddMintState::NotRequested,
            rec_ln_state: RecLNState::NotRequested,
            rec_ec_state: RecECState::NotRequested,
            send_ln_state: SendLNState::NotRequested,
            send_ec_state: SendECState::NotRequested,
            show_invoice_widget: ShowInvoiceWidget::new("RecLN".to_owned()),
            show_token_widget: ShowTokenWidget::new("SendECToken".to_owned()),
        }
    }

    fn event_listener() -> impl iced::futures::stream::Stream<Item = Message> {
        iced::stream::channel(100, |mut output| async move {
            // Create channel for getting events from outside
            let (inbound_sender, mut inbound_receiver) = mpsc::channel::<AppEvent>(100);

            let _res = output
                .send(Message::SubscriptionSender(inbound_sender))
                .await
                .unwrap();

            loop {
                match inbound_receiver.next().await {
                    None => {
                        println!("Error in Subscription: None (546)");
                    }
                    Some(ev) => {
                        // println!("Got AppEvent {:?}", ev);
                        let _res = output.send(Message::AppEvent(ev)).await;
                    }
                }
            }
        })
    }

    pub fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::run(Self::event_listener)
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        // println!("Update {:?}", message);
        match message {
            Message::SubscriptionSender(sender) => {
                let _res = self.app.init_with_sender(sender);
            }
            // Message::RefreshNoop => {}
            Message::RefreshInfo => {
                self.refresh_info();
            }
            Message::AppEvent(ev) => match ev {
                AppEvent::WalletInfo(wallet_info) => {
                    if let Ok(wallet_info) = &wallet_info {
                        self.wallet_info = Some(wallet_info.clone());
                    }
                }
                AppEvent::BalanceChange(balance) => {
                    if let Ok(balance) = &balance {
                        self.balance = Some(balance.clone());
                    }
                }
                AppEvent::MintsInfo(minfo) => {
                    if let Ok(minfo) = minfo {
                        self.mints_info = minfo;
                    }
                }
                AppEvent::BalanceAndWalletInfo(result) => {
                    if let Ok((balance_info, wallet_info)) = &result {
                        self.balance = Some(balance_info.clone());
                        self.wallet_info = Some(wallet_info.clone());
                    }
                }
                AppEvent::MintAdded(res) => {
                    self.add_mint_state = AddMintState::Completed(res);
                    self.refresh_info();
                }
                AppEvent::MintSelectedByUrl(_res) => {
                    self.refresh_info();
                }
                AppEvent::MintSelectedByIndex(_res) => {
                    self.refresh_info();
                }
                AppEvent::MintFromLnInvoice(invoice) => {
                    self.show_invoice_widget
                        .set_data(Some(invoice.clone()), None);
                    self.rec_ln_state = RecLNState::InvoiceReady(invoice)
                }
                AppEvent::MintFromLnRes(res) => {
                    self.show_invoice_widget.set_data(None, None);
                    self.rec_ln_state = RecLNState::Completed(res);
                    // TODO notification with amount
                    self.refresh_info();
                }
                AppEvent::ReceivedEC(res) => {
                    self.rec_ec_state = RecECState::Completed(res);
                    self.refresh_info();
                }
                AppEvent::MeltToLnRes(res) => {
                    self.send_ln_state = SendLNState::Completed(res);
                    self.refresh_info();
                }
                AppEvent::SendECRes(res) => {
                    match &res {
                        Ok((_sent, token)) => {
                            self.show_token_widget.set_data(Some(token.clone()), None);
                        }
                        Err(_err) => {
                            self.show_token_widget.set_data(None, None);
                        }
                    }
                    self.send_ec_state = SendECState::Completed(res);
                    // TODO notification with token
                    self.refresh_info();
                }
            },
            Message::Tab(tab) => {
                self.main_tab = tab;
            }
            Message::SelectMint(url) => {
                let _res = self.app.select_mint(url);
            }
            Message::AddMint(url) => {
                self.add_mint_state = AddMintState::Requested;
                let _res = self.app.add_mint(url);
            }
            Message::AmountInput(amount_str) => {
                if let Ok(amnt) = amount_str.parse::<f64>() {
                    self.amount_input = (amnt as u64).to_string();
                };
            }
            Message::InvoiceInput(invoice) => {
                self.invoice_input = invoice;
            }
            Message::TokenInput(token) => {
                self.token_input = token;
            }
            Message::AddMintInput(mint_url) => {
                self.add_mint_input = mint_url;
            }
            Message::ReceiveLN(amount) => {
                self.rec_ln_state = RecLNState::Requested(amount);
                let _res = self.app.mint_from_ln(amount);
            }
            Message::ReceiveLNOK => {
                self.rec_ln_state = RecLNState::NotRequested;
                self.show_invoice_widget.set_data(None, None);
            }
            Message::ReceiveEC(token) => {
                self.rec_ec_state = RecECState::Requested;
                let _res = self.app.receive_ec(token);
            }
            Message::ReceiveECOK => {
                self.rec_ec_state = RecECState::NotRequested;
                self.token_input.clear();
            }
            Message::SendLN(invoice) => {
                self.send_ln_state = SendLNState::Requested;
                let _res = self.app.melt_to_ln(invoice);
            }
            Message::SendLNOK => {
                self.send_ln_state = SendLNState::NotRequested;
                self.invoice_input.clear();
            }
            Message::SendEC(amount) => {
                self.send_ec_state = SendECState::Requested;
                let _res = self.app.send_ec(amount);
            }
            Message::SendECOK => {
                self.send_ec_state = SendECState::NotRequested;
                self.amount_input = "0".to_owned();
                self.show_token_widget.set_data(None, None);
            }
            Message::WidgetMessage((name, wmsg)) => {
                let _res = self.show_invoice_widget.update(&name, &wmsg);
                let _res = self.show_token_widget.update(&name, &wmsg);
            }
            Message::CopyToClipboard(text) => {
                println!("Copying to cloibpoard... ({}...)", &text[0..8]);
                return clipboard::write(text);
            }
        }
        Task::none()
    }

    pub fn view(&self) -> Element<Message> {
        self.view_main()
    }
}
