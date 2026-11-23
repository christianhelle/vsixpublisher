pub mod logger;
pub mod messages;
pub mod cli;
pub mod version;
pub mod enums;
pub mod manifest;
pub mod db;
pub mod commands;
pub mod client;
pub mod xml;
pub mod vsix;
pub mod validate;
pub mod marketplace_manifest;
pub mod package;
pub mod oidc;

use cli::Parser;
use client::HttpClientFactory;
use commands::{
    delete_extension::DeleteExtensionCommand, delete_publisher::DeletePublisherCommand, login::LoginCommand,
    logout::LogoutCommand, publish::PublishCommand,
};

/// Builds the parser with all commands, mirroring `Program.Main`.
pub fn parser() -> Parser {
    let db = db::default_path();
    let mut p = Parser::new();
    p.add(
        "publish",
        Box::new(PublishCommand {
            db_path: db.clone(),
            factory: Box::new(HttpClientFactory),
            tokens: Box::new(oidc::GitHubActionsTokenProvider::default()),
        }),
    );
    p.add(
        "deleteExtension",
        Box::new(DeleteExtensionCommand { db_path: db.clone(), factory: Box::new(HttpClientFactory) }),
    );
    p.add("login", Box::new(LoginCommand { db_path: db.clone(), factory: Box::new(HttpClientFactory) }));
    p.add("logout", Box::new(LogoutCommand { db_path: db.clone() }));
    p.add("deletePublisher", Box::new(DeletePublisherCommand { db_path: db, factory: Box::new(HttpClientFactory) }));
    p
}

/// Runs the tool with `args` (excluding the program name) and returns the exit code.
pub fn run(args: &[String]) -> i32 {
    let mut log = logger::Logger::stdio();
    parser().run(args, &mut log, &mut std::io::stderr())
}
