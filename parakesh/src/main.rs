use parakesh_common::pk_app::{BalanceInfo, WalletInfo};
use parakesh_common::pk_app_async::AppEvent;
use parakesh_common::{MintsSummary, PKAppAsync};

use std::io;
use std::io::Write;

fn get_status(app: &mut PKAppAsync) {
    let _res = app.get_balance_and_wallet_info();
}

fn cmd_help() {
    println!("\nAvailable commands:");
    println!("  help\t\t\t\tShows a list of commands.");
    println!("  quit | exit | q\t\tExit.");
    println!("");
    println!("  status\t\t\tShow wallet status.");
    println!("  listmints\t\t\tList used mints.");
    println!("  addmint <mint_url>\t\tAdd a mint.");
    println!("  selectmint <mint_number>\tSelect a mint, from known ones, by number, as listed in 'listminst', e.g. '1'; OR");
    println!("  selectmint <mint_url>\t\tSelect a mint, from known ones, by url.");
    println!("");
    println!("  recln <amount_sats>\t\tReceive LN, show LN invoice to-be-paid, for the specified amount, with the current mint.");
    println!("  sendln <ln_invoice>\t\tSend LN.");
    println!("  rec <ecash_token>\t\tReceive ecash");
    println!("  send <amount_sats>\t\tSend ecash, prepare ecash token for sending.");
    println!("");
}

fn cmd_status(app: &mut PKAppAsync) {
    get_status(app)
}

fn cmd_list_mints(app: &mut PKAppAsync) {
    let _res = app.get_mints_info();
}

fn cmd_addmint(app: &mut PKAppAsync, mint_url: &str) {
    let _res = app.add_mint(mint_url.to_owned());
}

fn cmd_selectmint_by_index(app: &mut PKAppAsync, mint_number: usize) {
    let _res = app.select_mint_by_index(mint_number);
}

fn cmd_selectmint_by_url(app: &mut PKAppAsync, mint_url: &str) {
    let _res = app.select_mint(mint_url.to_owned());
}

fn cmd_recln(app: &mut PKAppAsync, amount_sats: u64) {
    let _res = app.mint_from_ln(amount_sats);
}

fn cmd_sendln(app: &mut PKAppAsync, ln_invoice: &str) {
    let _res = app.melt_to_ln(ln_invoice.to_owned());
}

fn cmd_rec(app: &mut PKAppAsync, token: &str) {
    let _res = app.receive_ec(token.to_owned());
}

fn cmd_send(app: &mut PKAppAsync, amount_sats: u64) {
    let _res = app.send_ec(amount_sats);
}

fn print_prompt() {
    print!("> ");
    std::io::stdout().flush().unwrap(); // Without flushing, the `>` doesn't print
}

fn poll_for_user_input(app: &mut PKAppAsync) {
    println!("Enter \"help\" to view available commands. Press Ctrl-D to quit.");
    loop {
        print_prompt();

        let mut line = String::new();
        if let Err(e) = io::stdin().read_line(&mut line) {
            break println!("ERROR: {}", e);
        }

        if line.len() == 0 {
            // We hit EOF / Ctrl-D
            break;
        }

        let mut words = line.split_whitespace();
        if let Some(word) = words.next() {
            match word {
                "help" => cmd_help(),
                "quit" | "exit" | "q" => break,
                "status" => cmd_status(app),
                "listmints" => cmd_list_mints(app),

                "addmint" => {
                    let mint_url = if let Some(word) = words.next() {
                        word
                    } else {
                        println!("\nERROR: addmint requires <mint_url>");
                        continue;
                    };
                    cmd_addmint(app, mint_url);
                }

                "selectmint" => {
                    let mint_number_or_url = if let Some(word) = words.next() {
                        word
                    } else {
                        println!("\nERROR: selectmint requires <mint_number> OR <mint_url>");
                        continue;
                    };
                    match mint_number_or_url.parse::<usize>() {
                        Ok(mint_number) => cmd_selectmint_by_index(app, mint_number),
                        Err(_) => {
                            // could not parse parameter as number, assume url
                            cmd_selectmint_by_url(app, mint_number_or_url);
                        }
                    }
                }

                "recln" => {
                    let amount_str = if let Some(word) = words.next() {
                        word
                    } else {
                        println!("\nERROR: recln requires amount (in sats)");
                        continue;
                    };
                    let amount = match amount_str.parse::<u64>() {
                        Ok(amount) => amount,
                        Err(err) => {
                            println!(
                                "\nERROR: recln requires amount (in sats); {}",
                                err.to_string()
                            );
                            continue;
                        }
                    };
                    cmd_recln(app, amount);
                }

                "sendln" => {
                    let invoice_str = if let Some(word) = words.next() {
                        word
                    } else {
                        println!("\nERROR: sendln requires a LN invoice");
                        continue;
                    };
                    cmd_sendln(app, invoice_str);
                }

                "rec" => {
                    let token_str = if let Some(word) = words.next() {
                        word
                    } else {
                        println!("\nERROR: rec requires an ecash token");
                        continue;
                    };
                    cmd_rec(app, token_str);
                }

                "send" => {
                    let amount_str = if let Some(word) = words.next() {
                        word
                    } else {
                        println!("\nERROR: send requires amount (in sats");
                        continue;
                    };
                    let amount = match amount_str.parse::<u64>() {
                        Ok(amount) => amount,
                        Err(err) => {
                            println!(
                                "\nERROR: send requires amount (in sats); {}",
                                err.to_string()
                            );
                            continue;
                        }
                    };
                    cmd_send(app, amount);
                }

                _ => println!("Unknown command. See `\"help\" for available commands."),
            }
        }

        get_status(app);
    }
}

fn print_balance_and_wallet_info(balance_info: Option<&BalanceInfo>, wallet_info: &WalletInfo) {
    if wallet_info.is_inititalized {
        print!("Wallet: OK");
    } else {
        print!("Wallet: NOT INITIALIZED");
    }
    print!(" \t ");
    if let Some(balance) = balance_info {
        print!("Balance: {} sats", balance.0);
    }
    print!(" \t ");
    match &wallet_info.mints_summary {
        MintsSummary::None => print!("No mints"),
        MintsSummary::Single(mint) => print!("Mint: {}", mint),
        MintsSummary::Multiple(cnt) => print!("{} mints", cnt),
    }
    print!(" \t ");
    print!("Selected: {}", wallet_info.selected_mint_url);
    print!(" \t ");
    println!("");
}

fn handle_event(event: AppEvent) {
    // println!("Got AppEvent {:?}", event);
    match event {
        AppEvent::BalanceChange(balance_info) => match balance_info {
            Ok(balance) => println!("Balance: {} sats", balance.0),
            Err(err) => println!("\nERROR retrieving balance! {}", err),
        },
        AppEvent::WalletInfo(wallet_info) => match wallet_info {
            Ok(wallet_info) => {
                print_balance_and_wallet_info(None, &wallet_info);
            }
            Err(err) => println!("\nERROR retrieving wallet info! {}", err),
        },
        AppEvent::BalanceAndWalletInfo(result) => match result {
            Ok((balance_info, wallet_info)) => {
                print_balance_and_wallet_info(Some(&balance_info), &wallet_info);
            }
            Err(err) => println!("\nERROR retrieving balance/wallet info! {}", err),
        },
        AppEvent::MintsInfo(mint_info) => match mint_info {
            Ok(mints) => {
                if mints.is_empty() {
                    println!("No mints used.");
                } else {
                    println!("Mints used: ({})", mints.len());
                    for (i, mint) in mints.iter().enumerate() {
                        println!("    {}\t{}\t{}", i + 1, mint.url, mint.balance);
                    }
                }
            }
            Err(err) => println!("\nERROR retrieving mints info {}", err.to_string()),
        },
        AppEvent::MintAdded(res) => match res {
            Ok(_) => println!("Mint added"),
            Err(err) => println!("\nERROR adding mint {}", err.to_string()),
        },
        AppEvent::MintSelectedByUrl(res) => match res {
            Ok(url) => println!("Mint selected: {}", url),
            Err(err) => println!("\nERROR selecting mint {}", err.to_string()),
        },
        AppEvent::MintSelectedByIndex(res) => match res {
            Ok(index) => println!("Mint selected: {}", index),
            Err(err) => println!("\nERROR selecting mint {}", err.to_string()),
        },
        AppEvent::MintFromLnRes(res) => match res {
            Ok(minted) => println!("Received LN, got ecash for {} sats", minted),
            Err(err) => println!("\nERROR in receive LN {}", err.to_string()),
        },
        AppEvent::MintFromLnInvoice(invoice) => println!("Pay the invoice!\n\n{}\n", invoice),
        AppEvent::MeltToLnRes(res) => match res {
            Ok(sent) => println!("Sent LN, amount {} sats", sent),
            Err(err) => println!("\nERROR in send LN {}", err.to_string()),
        },
        AppEvent::ReceivedEC(res) => match res {
            Ok(received) => println!("Received ecash for {} sats", received),
            Err(err) => println!("\nERROR in receive {}", err.to_string()),
        },
        AppEvent::SendECRes(res) => match res {
            Ok((amount, token)) => println!(
                "Prepared token for sending, amount {} (sats):\n\n{}\n",
                amount, token,
            ),
            Err(err) => println!("\nERROR in send {}", err.to_string()),
        },
    }
    // for nicer console reading
    print_prompt();
}

#[tokio::main]
async fn main() {
    println!("ParaKesh: GM!");
    // TODO args, etc.

    let mut app = PKAppAsync::new_with_callback(handle_event).unwrap();

    // handle interactive commands
    poll_for_user_input(&mut app);

    println!("ParaKesh: ciao!");
}
