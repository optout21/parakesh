use parakesh_common::{MintsSummary, PKApp};

use std::io;
use std::io::Write;

async fn print_status(app: &PKApp) {
    let info = match app.get_wallet_info().await {
        Ok(info) => info,
        Err(err) => {
            println!("\nERROR retrieving wallet info! {}", err);
            return;
        }
    };
    if info.is_inititalized {
        print!("Wallet: OK");
    } else {
        print!("Wallet: NOT INITIALIZED");
    }
    print!(" \t ");
    match app.get_balance().await {
        Ok(balance) => print!("Balance: {} sats", balance.0),
        Err(_err) => print!("Balance: ERR"),
    }
    print!(" \t ");
    match &info.mints_summary {
        MintsSummary::None => print!("No mints"),
        MintsSummary::Single(mint) => print!("Mint: {}", mint),
        MintsSummary::Multiple(cnt) => print!("{} mints", cnt),
    }
    print!(" \t ");
    print!("Selected: {}", info.selected_mint_url);
    print!(" \t ");
    println!("");
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

async fn cmd_status(app: &PKApp) {
    print_status(app).await
}

async fn cmd_list_mints(app: &PKApp) {
    let mints = match app.get_mints_info().await {
        Ok(mints) => mints,
        Err(err) => {
            println!("\nERROR: {}", err.to_string());
            return;
        }
    };
    if mints.is_empty() {
        println!("No mints used.");
    } else {
        println!("Mints used: ({})", mints.len());
        for (i, mint) in mints.iter().enumerate() {
            println!("    {}\t{}\t{}", i + 1, mint.url, mint.balance);
        }
    }
}

async fn cmd_addmint(app: &mut PKApp, mint_url: &str) {
    match app.add_mint(mint_url).await {
        Ok(_) => {}
        Err(err) => {
            println!("\nERROR adding mint {} {}", mint_url, err.to_string());
            return;
        }
    }
    println!("Selected mint: {}", app.selected_mint());
}

async fn cmd_selectmint_by_index(app: &mut PKApp, mint_number: usize) {
    match app.select_mint_by_index(mint_number).await {
        Ok(_) => println!("Selected mint: {}", app.selected_mint()),
        Err(err) => println!(
            "\nERROR selecting mint {}; {}",
            mint_number,
            err.to_string()
        ),
    }
}

async fn cmd_selectmint_by_url(app: &mut PKApp, mint_url: &str) {
    match app.select_mint(mint_url).await {
        Ok(_) => {}
        Err(err) => {
            println!("\nERROR selecting mint {} {}", mint_url, err.to_string());
            return;
        }
    }
    println!("Selected mint: {}", app.selected_mint());
}

async fn cmd_recln(app: &mut PKApp, amount_sats: u64) {
    match app.mint_from_ln_start(amount_sats).await {
        Ok((invoice, intermediary_result)) => {
            println!("Pay the invoice: {} !", invoice);
            match app.mint_from_ln_wait(intermediary_result).await {
                Ok(minted) => println!(
                    "Received LN, got ecash for {} sats, with mint {}",
                    minted,
                    app.selected_mint()
                ),
                Err(err) => println!("\nERROR receiving LN, {}", err),
            }
        }
        Err(err) => println!("\nERROR receiving LN, {}", err),
    }
}

async fn cmd_sendln(app: &mut PKApp, ln_invoice: &str) {
    match app.melt_to_ln(ln_invoice).await {
        Ok(sent) => println!(
            "Sent LN, amount {} sats, from mint {}",
            sent,
            app.selected_mint()
        ),
        Err(err) => println!("\nERROR sending LN, {}", err.to_string()),
    }
}

async fn cmd_rec(app: &mut PKApp, token: &str) {
    match app.receive_ecash(token).await {
        Ok(received) => println!("Received ecash for {} sats", received,),
        Err(err) => println!("\nERROR receiving, {}", err.to_string()),
    }
}

async fn cmd_send(app: &mut PKApp, amount_sats: u64) {
    match app.send_ecash(amount_sats).await {
        Ok((_sent, token)) => println!(
            "Prepared token for sending, amount {} (sats):\n\n{}\n",
            amount_sats, token,
        ),
        Err(err) => println!("\nERROR in send, {}", err.to_string()),
    }
}

async fn poll_for_user_input(app: &mut PKApp) {
    println!("Enter \"help\" to view available commands. Press Ctrl-D to quit.");
    loop {
        print_status(&app).await;

        print!("> ");
        std::io::stdout().flush().unwrap(); // Without flushing, the `>` doesn't print

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
                "status" => cmd_status(app).await,
                "listmints" => cmd_list_mints(app).await,

                "addmint" => {
                    let mint_url = if let Some(word) = words.next() {
                        word
                    } else {
                        println!("\nERROR: addmint requires <mint_url>");
                        continue;
                    };
                    cmd_addmint(app, mint_url).await;
                }

                "selectmint" => {
                    let mint_number_or_url = if let Some(word) = words.next() {
                        word
                    } else {
                        println!("\nERROR: selectmint requires <mint_number> OR <mint_url>");
                        continue;
                    };
                    match mint_number_or_url.parse::<usize>() {
                        Ok(mint_number) => cmd_selectmint_by_index(app, mint_number).await,
                        Err(_) => {
                            // could not parse parameter as number, assume url
                            cmd_selectmint_by_url(app, mint_number_or_url).await;
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
                    cmd_recln(app, amount).await;
                }

                "sendln" => {
                    let invoice_str = if let Some(word) = words.next() {
                        word
                    } else {
                        println!("\nERROR: sendln requires a LN invoice");
                        continue;
                    };
                    cmd_sendln(app, invoice_str).await;
                }

                "rec" => {
                    let token_str = if let Some(word) = words.next() {
                        word
                    } else {
                        println!("\nERROR: rec requires an ecash token");
                        continue;
                    };
                    cmd_rec(app, token_str).await;
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
                    cmd_send(app, amount).await;
                }

                "test" => {
                    print_status(&app).await;

                    // let _res1 = app.initialize().unwrap();
                    let minted = app
                        .mint_from_ln(10, |invoice| {
                            println!("\nInvoice: {}\n", invoice);
                        })
                        .await
                        .unwrap();
                    println!("Minted {}", minted);

                    print_status(&app).await;

                    let (_sent, token) = app.send_ecash(10).await.unwrap();
                    println!("Prepared for send {}", token);

                    print_status(&app).await;
                }

                _ => println!("Unknown command. See `\"help\" for available commands."),
            }
        }
    }
}

#[tokio::main]
async fn main() {
    println!("Parakesh: GM!");

    // args, init, etc.

    let mut app = PKApp::new().await.unwrap();

    // handle interactive commands
    poll_for_user_input(&mut app).await;

    // event_loop_handle.abort();

    // stop, etc.

    // while !bg_handle.is_finished() {
    //     std::thread::sleep(Duration::from_millis(10));
    // }
    println!("Parakesh: ciao!");
}
