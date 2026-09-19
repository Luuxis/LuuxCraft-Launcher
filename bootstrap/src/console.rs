//! Déroulé en console, hors Windows.
//!
//! Ce crate n'est jamais distribué ailleurs que sous Windows, où il ouvre une
//! fenêtre (`win::gui`). Cette console existe pour qu'il se compile, se teste
//! et s'exécute sur une machine de développement Linux ou macOS, avec le même
//! déroulé et les mêmes messages.

use std::io::Write;

use crate::install::{self, Options};
use crate::net::Http;
use crate::report::{self, Report, STEPS};

struct Console;

impl Report for Console {
    fn step(&self, index: u8, label: &str) {
        println!("[{index}/{STEPS}] {label}");
    }

    fn detail(&self, text: &str) {
        println!("      {text}");
    }

    fn transfer(&self, label: &str, done: u64, total: u64) {
        // Réécrite en place : une seule ligne par téléchargement.
        print!("\r      {}   ", report::transfer_text(label, done, total));
        let _ = std::io::stdout().flush();
    }

    fn transfer_done(&self) {
        println!();
    }
}

pub fn run() -> i32 {
    println!("Installation du launcher");

    let http = Http::new();
    let console = Console;
    let outcome = install::prepare(&http, &console).and_then(|prepared| {
        console.detail(&format!("dossier : {}", prepared.layout.root.display()));
        let options = Options {
            desktop_shortcut: true,
        };
        let target = install::install(&http, &prepared, &options, &console)?;
        install::launch(&prepared, &target, &console)
    });

    match outcome {
        Ok(()) => 0,
        Err(error) => {
            eprintln!();
            eprintln!("{error}");
            eprintln!();
            1
        }
    }
}
