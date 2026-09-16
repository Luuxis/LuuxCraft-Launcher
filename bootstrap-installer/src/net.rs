//! Accès HTTP et affichage de la progression.
//!
//! Un seul agent est partagé : la connexion TLS ouverte pour le manifeste sert
//! ensuite aux téléchargements, ce qui évite une poignée de main par fichier du
//! pack client.
//!
//! Les délais sont posés par lecture, pas globalement : un moteur de 50 Mo sur
//! une ligne lente doit pouvoir prendre dix minutes, mais un serveur muet doit
//! rendre la main en une minute.

use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::config::USER_AGENT;
use crate::error::{self, BootstrapError, Result};
use crate::hashing;

pub struct Http {
    agent: ureq::Agent,
}

impl Http {
    pub fn new() -> Self {
        let agent = ureq::AgentBuilder::new()
            .user_agent(USER_AGENT)
            .timeout_connect(Duration::from_secs(20))
            .timeout_read(Duration::from_secs(60))
            .build();
        Self { agent }
    }

    pub fn get_text(&self, url: &str) -> Result<String> {
        let response = self
            .agent
            .get(url)
            .call()
            .map_err(|failure| error::from_http(url, failure))?;
        response.into_string().map_err(|failure| {
            BootstrapError::new(format!("réponse illisible du panel ({failure}).\n{url}"))
                .hint("Réessayez dans quelques instants.")
        })
    }

    /// Télécharge vers `destination` en calculant l'empreinte au passage, et
    /// rend l'empreinte obtenue.
    ///
    /// Le fichier visé est toujours un chemin temporaire : rien n'est mis en
    /// place tant que l'appelant n'a pas comparé cette empreinte à celle du
    /// manifeste. C'est ce qui garantit qu'un moteur à moitié écrit — coupure
    /// réseau, disque plein — n'est jamais lancé.
    pub fn download(
        &self,
        url: &str,
        destination: &Path,
        expected_size: u64,
        label: &str,
    ) -> Result<String> {
        let response = self
            .agent
            .get(url)
            .call()
            .map_err(|failure| error::from_http(url, failure))?;
        let mut reader = response.into_reader();

        let mut file = File::create(destination)
            .map_err(|error| BootstrapError::io("La création du fichier", destination, &error))?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0u8; 64 * 1024];
        let mut progress = Progress::new(label, expected_size);
        let mut written: u64 = 0;

        loop {
            let read = reader.read(&mut buffer).map_err(|failure| {
                BootstrapError::new(format!("le téléchargement a été interrompu ({failure})."))
                    .hint("Vérifiez votre connexion internet puis relancez l'installation.")
            })?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            file.write_all(&buffer[..read])
                .map_err(|error| BootstrapError::io("L'écriture", destination, &error))?;
            written += read as u64;
            progress.tick(written);
        }

        // Sans `sync_all`, le renommage qui suit pourrait publier un fichier
        // dont le contenu n'a pas encore atteint le disque.
        file.sync_all()
            .map_err(|error| BootstrapError::io("L'écriture", destination, &error))?;
        progress.finish(written);

        Ok(hashing::to_hex(&hasher.finalize()[..]))
    }
}

/// Barre de progression sur une seule ligne, réécrite en place.
struct Progress {
    label: String,
    total: u64,
    last_percent: u64,
    last_draw: Instant,
    drawn: bool,
}

const MEGABYTE: f64 = 1024.0 * 1024.0;

impl Progress {
    fn new(label: &str, total: u64) -> Self {
        Self {
            label: label.to_owned(),
            total,
            last_percent: u64::MAX,
            last_draw: Instant::now(),
            drawn: false,
        }
    }

    fn tick(&mut self, done: u64) {
        let percent = if self.total > 0 {
            (done.saturating_mul(100) / self.total).min(100)
        } else {
            0
        };
        // Redessiner à chaque bloc de 64 Ko saturerait un terminal lent.
        let due = percent != self.last_percent
            || self.last_draw.elapsed() >= Duration::from_millis(250);
        if !due {
            return;
        }
        self.last_percent = percent;
        self.last_draw = Instant::now();
        self.draw(done, percent);
    }

    fn draw(&mut self, done: u64, percent: u64) {
        let done_mb = done as f64 / MEGABYTE;
        if self.total > 0 {
            print!(
                "\r      {} {:>3} %  {:.1} / {:.1} Mo   ",
                self.label,
                percent,
                done_mb,
                self.total as f64 / MEGABYTE
            );
        } else {
            print!("\r      {} {:.1} Mo   ", self.label, done_mb);
        }
        let _ = std::io::stdout().flush();
        self.drawn = true;
    }

    fn finish(&mut self, done: u64) {
        if self.drawn {
            self.draw(done, 100);
            println!();
        }
    }
}
