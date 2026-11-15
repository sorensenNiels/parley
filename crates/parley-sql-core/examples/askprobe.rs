//! Asks the SQL assistant's provider a question without the UI or an IBM i (R-SQL-48): the
//! model's catalog lookups are answered from a small made-up library, DEMO. For checking a
//! provider by hand, e.g. a local llama-server:
//!
//! `cargo run -p parley-core --example askprobe -- llama-cpp http://127.0.0.1:8080/v1 "question"`
//!
//! The key, for providers that need one, comes from the provider's environment variable.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "an example reports to the terminal"
)]

use parley_sql_core::assistant::{
    self, AssistantConfig, Chat, Lookup, MAX_ROUNDS, Provider, Reply,
};

/// What the made-up catalog says about a lookup.
fn catalog(lookup: &Lookup) -> String {
    match lookup {
        Lookup::Tables { library } if library.eq_ignore_ascii_case("DEMO") => {
            "KUNDER (table): Customers\nORDRER (table): Orders".to_owned()
        }
        Lookup::Columns { library, table } if library.eq_ignore_ascii_case("DEMO") => {
            match table.to_uppercase().as_str() {
                "KUNDER" => "KUNDENR INTEGER not null: Customer number\n\
                             NAVN VARCHAR(50): Name\nBY VARCHAR(30): City"
                    .to_owned(),
                "ORDRER" => "ORDRENR INTEGER not null: Order number\n\
                             KUNDENR INTEGER not null: Customer number\n\
                             BELOB DECIMAL(11,2): Amount\nSTATUS CHAR(1): O open, C closed"
                    .to_owned(),
                _ => format!("No table {table} in {library}."),
            }
        }
        Lookup::Tables { library } | Lookup::Columns { library, .. } => {
            format!("No library {library}.")
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let provider = args.next().unwrap_or_else(|| "llama-cpp".to_owned());
    let Some(provider) = Provider::ALL.into_iter().find(|p| p.id() == provider) else {
        eprintln!("askprobe: unknown provider '{provider}'");
        std::process::exit(2);
    };
    let config = AssistantConfig {
        provider,
        url: args.next().unwrap_or_default(),
        model: std::env::var("ASK_MODEL").unwrap_or_default(),
    };
    let question = args
        .next()
        .unwrap_or_else(|| "Customers in Århus with open orders".to_owned());
    let key = assistant::key_from_environment(provider);
    let mut chat = Chat::new(
        config.clone(),
        &question,
        "V7R5M0",
        "DEMO",
        &["DEMO".to_owned()],
    );
    println!(
        "asking {} at {}: {question}",
        provider.label(),
        config.url()
    );
    for round in 1..=MAX_ROUNDS {
        let started = std::time::Instant::now();
        let reply = assistant::send(&config, key.as_deref(), &chat.body())
            .and_then(|value| chat.read(&value));
        let took = started.elapsed().as_secs_f32();
        match reply {
            Err(e) => {
                eprintln!("round {round} ({took:.1} s): {e}");
                std::process::exit(1);
            }
            Ok(Reply::Sql { sql, explanation }) => {
                println!("round {round} ({took:.1} s): answer\n-- {explanation}\n{sql}");
                return;
            }
            Ok(Reply::Text(text)) => {
                println!("round {round} ({took:.1} s): words only\n{text}");
                return;
            }
            Ok(Reply::Lookups { turn, calls }) => {
                let answers: Vec<(String, String)> = calls
                    .iter()
                    .map(|(id, lookup)| {
                        println!("round {round} ({took:.1} s): looks up {lookup:?}");
                        (id.clone(), catalog(lookup))
                    })
                    .collect();
                chat.answer(turn, &answers);
            }
        }
    }
    eprintln!("gave up after {MAX_ROUNDS} rounds");
    std::process::exit(1);
}
