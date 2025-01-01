use std::{env, sync::Arc};

use clap::{Parser, Subcommand};
use odr_core::{
    keys::KeyManager,
    store::{
        keys::{SqliteStore as KeyStore, Store as _},
        user::{self, PasswordType, SqliteStore as UserStore, Store as _, User},
    },
    user::hash_password,
};
use sqlx::{migrate::MigrateDatabase as _, Sqlite, SqlitePool};

#[derive(Parser)]
enum Commands {
    Migrate,
    Rotate {
        #[clap(long)]
        clear: bool,

        #[clap(long, conflicts_with("clear"))]
        noclear: bool,

        #[clap(long)]
        nointeractive: bool,
    },

    User {
        #[clap(subcommand)]
        subcmd: UserSubcommand,
    },

    Init {
        #[clap(long)]
        email: Option<String>,

        #[clap(long)]
        password: Option<String>,

        #[clap(long)]
        display_name: Option<String>,

        #[clap(long)]
        nointeractive: bool,
    },
}

#[derive(Subcommand)]
enum UserSubcommand {
    Add {
        #[clap(long)]
        email: Option<String>,

        #[clap(long)]
        password: Option<String>,

        #[clap(long)]
        display_name: Option<String>,

        #[clap(long)]
        nointeractive: bool,
    },

    SetPassword {
        #[clap(long)]
        email: Option<String>,

        #[clap(long)]
        password: Option<String>,

        #[clap(long)]
        nointeractive: bool,
    },
}

enum ClearKey {
    Yes,
    No,
    NotSet,
}

fn db_url() -> String {
    format!("sqlite://{}/odr-sqlite.db", env::temp_dir().display())
}

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let cli = Commands::parse();
    match cli {
        Commands::Migrate => {
            migrate().await?;
        }
        Commands::Rotate {
            clear,
            noclear,
            nointeractive,
        } => {
            let clear = match (clear, noclear) {
                (true, false) => ClearKey::Yes,
                (false, true) => ClearKey::No,
                (false, false) => ClearKey::NotSet,
                (true, true) => unreachable!(),
            };

            let db_url = db_url();
            let db = Arc::new(SqlitePool::connect(&db_url).await?);

            rotate_key(db, clear, !nointeractive).await?;
        }
        Commands::User { subcmd } => match subcmd {
            UserSubcommand::Add {
                email,
                password,
                display_name,
                nointeractive,
            } => {
                let db_url = db_url();
                let db = Arc::new(SqlitePool::connect(&db_url).await?);
                add_user(db, email, password, display_name, !nointeractive).await?;
            }
            UserSubcommand::SetPassword {
                email,
                password,
                nointeractive,
            } => {
                set_password(email, password, !nointeractive).await?;
            }
        },
        Commands::Init {
            email,
            password,
            display_name,
            nointeractive,
        } => {
            init(email, password, display_name, !nointeractive).await?;
        }
    };

    Ok(())
}

async fn migrate() -> Result<(), anyhow::Error> {
    let db_url = db_url();
    if !Sqlite::database_exists(&db_url).await.unwrap_or(false) {
        Sqlite::create_database(&db_url).await?;
    }

    let db = Arc::new(SqlitePool::connect(&db_url).await?);
    sqlx::migrate!("../migrations").run(&(*db)).await?;
    Ok(())
}

async fn rotate_key(
    db: Arc<SqlitePool>,
    clear: ClearKey,
    interactive: bool,
) -> Result<(), anyhow::Error> {
    let key_store = Arc::new(KeyStore::new(db.clone()));
    let key_manager = KeyManager::new(key_store);
    let clear = match clear {
        ClearKey::Yes => true,
        ClearKey::No => false,
        ClearKey::NotSet => {
            if interactive {
                inquire::Confirm::new("Do you want to clear old keys?")
                    .with_default(false)
                    .prompt()?
            } else {
                return Err(anyhow::anyhow!(
                    "--nointeractive set, --noclear or --clear must be set"
                ));
            }
        }
    };
    key_manager.rotate_key(clear).await?;
    Ok(())
}

async fn add_user(
    db: Arc<SqlitePool>,
    email: Option<String>,
    password: Option<String>,
    display_name: Option<String>,
    interactive: bool,
) -> Result<(), anyhow::Error> {
    let user_store = UserStore::new(db.clone());
    let email = match email {
        Some(email) => email,
        None => {
            if interactive {
                inquire::Text::new("Email").prompt()?
            } else {
                return Err(anyhow::anyhow!("--nointeractive set, --email must be set"));
            }
        }
    };

    let password = match password {
        Some(password) => password,
        None => {
            if interactive {
                inquire::Password::new("Password").prompt()?
            } else {
                return Err(anyhow::anyhow!(
                    "--nointeractive set, --password must be set"
                ));
            }
        }
    };

    let display_name = match display_name {
        Some(display_name) => display_name,
        None => {
            if interactive {
                inquire::Text::new("Display Name").prompt()?
            } else {
                return Err(anyhow::anyhow!(
                    "--nointeractive set, --display_name must be set"
                ));
            }
        }
    };

    let hashed_password =
        hash_password(&password).map_err(|e| anyhow::anyhow!(format!("{}", e)))?;

    user_store
        .upsert(vec![User {
            id: "".to_owned(),
            display_name,
            email,
            password: PasswordType::Set(hashed_password),
        }])
        .await?;
    Ok(())
}

async fn set_password(
    email: Option<String>,
    password: Option<String>,
    interactive: bool,
) -> Result<(), anyhow::Error> {
    let db_url = db_url();
    let db = Arc::new(SqlitePool::connect(&db_url).await?);
    let user_store = UserStore::new(db.clone());
    let email = match email {
        Some(email) => email,
        None => {
            if interactive {
                inquire::Text::new("Email").prompt()?
            } else {
                return Err(anyhow::anyhow!("--nointeractive set, --email must be set"));
            }
        }
    };

    let mut user = user_store
        .query(Some(&user::Query::Email(user::EmailQuery::Equals(
            email.clone(),
        ))))
        .await?;

    let mut user = match user.pop() {
        Some(user) => user,
        None => {
            return Err(anyhow::anyhow!("User not found"));
        }
    };

    let password = match password {
        Some(password) => password,
        None => {
            if interactive {
                inquire::Password::new("Password").prompt()?
            } else {
                return Err(anyhow::anyhow!(
                    "--nointeractive set, --password must be set"
                ));
            }
        }
    };

    let hashed_password =
        hash_password(&password).map_err(|e| anyhow::anyhow!(format!("{}", e)))?;

    user.password = PasswordType::Set(hashed_password);

    user_store.upsert(vec![user]).await?;
    Ok(())
}

async fn init(
    email: Option<String>,
    password: Option<String>,
    display_name: Option<String>,
    interactive: bool,
) -> Result<(), anyhow::Error> {
    println!("Initializing database");
    migrate().await?;

    let db_url = db_url();
    let db = Arc::new(SqlitePool::connect(&db_url).await?);
    let key_store = KeyStore::new(db.clone());
    match key_store.has().await {
        Ok(true) => (),
        Ok(false) => {
            println!("No keys found, generating new keys");
            rotate_key(db.clone(), ClearKey::No, interactive).await?;
        }
        Err(e) => return Err(e.into()),
    };

    let user_store = UserStore::new(db.clone());
    if user_store.query(None).await?.is_empty() {
        println!("No users found, adding new user");
        add_user(db, email, password, display_name, interactive).await?;
    }

    Ok(())
}
