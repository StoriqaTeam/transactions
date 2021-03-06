#[macro_use]
extern crate clap;
extern crate transactions_lib;

use clap::App;

fn main() {
    let yaml = load_yaml!("cli.yml");
    let mut app = App::from_yaml(yaml);
    let matches = app.clone().get_matches();

    if let Some(_) = matches.subcommand_matches("config") {
        transactions_lib::print_config();
    } else if let Some(_) = matches.subcommand_matches("server") {
        transactions_lib::start_server();
    } else if let Some(matches) = matches.subcommand_matches("create_user") {
        let name = matches.value_of("name").unwrap();
        transactions_lib::create_user(&name);
    } else if let Some(matches) = matches.subcommand_matches("repair_approval_pending_transaction") {
        let id = matches.value_of("id").unwrap();
        transactions_lib::repair_approval_pending_transaction(&id);
    } else if let Some(matches) = matches.subcommand_matches("repair_withdrawal_pending_transaction") {
        let id = matches.value_of("id").unwrap();
        transactions_lib::repair_withdrawal_pending_transaction(&id);
    } else {
        let _ = app.print_help();
        println!("\n")
    }
}
