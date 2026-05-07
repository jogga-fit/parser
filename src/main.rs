// Avoid musl's default allocator due to lackluster performance
// https://nickb.dev/blog/default-musl-allocator-considered-harmful-to-performance
#[cfg(all(not(target_arch = "wasm32"), target_env = "musl"))]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "server")]
fn main() {
    use std::path::Path;

    use clap::Parser;
    use jogga::{
        db::pool::{DbConfig, create_pool},
        server::{
            cli::{Cli, Commands, SeedOwnerArgs, seed_owner},
            config::AppConfig,
        },
        web::server::run_server,
    };

    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            let cli = Cli::parse();

            match cli.command {
                Some(Commands::SeedOwner { password }) => {
                    let config_path = cli.config.as_deref().map(Path::new);
                    let config = AppConfig::load(config_path).expect("failed to load config");

                    let db_config = DbConfig {
                        url: config.database.url.clone(),
                        max_connections: config.database.max_connections,
                    };
                    let pool = create_pool(&db_config).await.expect("failed to build pool");

                    let args = SeedOwnerArgs {
                        username: config.owner.username.clone(),
                        email: config.owner.contact.clone(),
                        password,
                        domain: config.instance.domain.clone(),
                    };

                    seed_owner(&pool, args).await.expect("seed-owner failed");
                    println!("Owner account seeded successfully.");
                }
                None => {
                    run_server(cli.config.as_deref().map(Path::new)).await;
                }
            }
        });
}

#[cfg(not(feature = "server"))]
fn main() {
    dioxus::launch(jogga::web::app::App);
}
