// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use clap::Parser;
use cc_proxy_lib::cli::Cli;

fn main() {
    let cli = Cli::parse();

    if let Err(e) = cc_proxy_lib::run_cli(cli) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}