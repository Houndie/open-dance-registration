use clap::{Parser, Subcommand};
use odr_server::{
    authentication::create_token,
    keys::{KeyManager as _, StoreKeyManager},
    proto::{permission_role, EventRole, OrganizationRole, Permission, PermissionRole},
    store::{
        keys::{SqliteStore as KeyStore, Store as _},
        permission::{SqliteStore as PermissionStore, Store as _},
        user::{self, PasswordType, SqliteStore as UserStore, Store as _, User},
    },
    user::hash_password,
};
use sqlx::{migrate::MigrateDatabase as _, Sqlite, SqlitePool};
use std::{env, sync::Arc};

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

    Permission {
        #[clap(subcommand)]
        subcmd: PermissionSubcommand,
    },

    Init {
        #[clap(long)]
        username: Option<String>,

        #[clap(long)]
        password: Option<String>,

        #[clap(long)]
        nointeractive: bool,

        #[clap(long)]
        email: Option<String>,
    },

    Login {
        #[clap(long)]
        username: Option<String>,

        #[clap(long)]
        password: Option<String>,

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
        username: Option<String>,

        #[clap(long)]
        nointeractive: bool,
    },

    SetPassword {
        #[clap(long)]
        username: Option<String>,

        #[clap(long)]
        password: Option<String>,

        #[clap(long)]
        nointeractive: bool,
    },
}

#[derive(Subcommand)]
enum PermissionSubcommand {
    Add {
        #[clap(long)]
        username: Option<String>,

        #[clap(long)]
        permission: Option<String>,

        #[clap(long)]
        id: Option<String>,

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
                username,
                nointeractive,
            } => {
                let db_url = db_url();
                let db = Arc::new(SqlitePool::connect(&db_url).await?);
                add_user(db, email, password, username, !nointeractive).await?;
            }
            UserSubcommand::SetPassword {
                username,
                password,
                nointeractive,
            } => {
                set_password(username, password, !nointeractive).await?;
            }
        },
        Commands::Permission { subcmd } => match subcmd {
            PermissionSubcommand::Add {
                username,
                permission,
                id,
                nointeractive,
            } => {
                add_permission(username, permission, id, !nointeractive).await?;
            }
        },
        Commands::Login {
            username,
            password,
            nointeractive,
        } => {
            login(username, password, !nointeractive).await?;
        }
        Commands::Init {
            email,
            password,
            username,
            nointeractive,
        } => {
            init(email, password, username, !nointeractive).await?;
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
    sqlx::migrate!("./migrations").run(&(*db)).await?;
    Ok(())
}

async fn rotate_key(
    db: Arc<SqlitePool>,
    clear: ClearKey,
    interactive: bool,
) -> Result<(), anyhow::Error> {
    let key_store = Arc::new(KeyStore::new(db.clone()));
    let key_manager = StoreKeyManager::new(key_store);
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
    username: Option<String>,
    interactive: bool,
) -> Result<String, anyhow::Error> {
    let user_store = UserStore::new(db.clone());

    let username = match username {
        Some(username) => username,
        None => {
            if interactive {
                inquire::Text::new("Username").prompt()?
            } else {
                return Err(anyhow::anyhow!(
                    "--nointeractive set, --username must be set"
                ));
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

    let hashed_password =
        hash_password(&password).map_err(|e| anyhow::anyhow!(format!("{}", e)))?;

    let mut user = user_store
        .upsert(vec![User {
            id: "".to_owned(),
            username,
            email,
            password: PasswordType::Set(hashed_password),
        }])
        .await?;

    Ok(user.remove(0).id)
}

async fn set_password(
    username: Option<String>,
    password: Option<String>,
    interactive: bool,
) -> Result<(), anyhow::Error> {
    let db_url = db_url();
    let db = Arc::new(SqlitePool::connect(&db_url).await?);
    let user_store = UserStore::new(db.clone());
    let username = match username {
        Some(username) => username,
        None => {
            if interactive {
                inquire::Text::new("Email").prompt()?
            } else {
                return Err(anyhow::anyhow!("--nointeractive set, --email must be set"));
            }
        }
    };

    let mut user = user_store
        .query(Some(&user::Query::Username(user::UsernameQuery::Equals(
            username.clone(),
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

async fn add_permission(
    username: Option<String>,
    permission: Option<String>,
    id: Option<String>,
    interactive: bool,
) -> Result<(), anyhow::Error> {
    let db_url = db_url();
    let db = Arc::new(SqlitePool::connect(&db_url).await?);
    let user_store = UserStore::new(db.clone());
    let permission_store = PermissionStore::new(db.clone());

    let username = match username {
        Some(username) => username,
        None => {
            if interactive {
                inquire::Text::new("Email").prompt()?
            } else {
                return Err(anyhow::anyhow!("--nointeractive set, --email must be set"));
            }
        }
    };

    let mut user = user_store
        .query(Some(&user::Query::Username(user::UsernameQuery::Equals(
            username.clone(),
        ))))
        .await?;

    let user = match user.pop() {
        Some(user) => user,
        None => {
            return Err(anyhow::anyhow!("User not found"));
        }
    };

    let permission = match permission {
        Some(permission) => permission,
        None => {
            if interactive {
                inquire::Text::new("Permission").prompt()?
            } else {
                return Err(anyhow::anyhow!(
                    "--nointeractive set, --permission must be set"
                ));
            }
        }
    };

    let get_id = || match id {
        Some(id) => Ok(id),
        None => {
            if interactive {
                inquire::Text::new("ID").prompt().map_err(|e| e.into())
            } else {
                Err(anyhow::anyhow!("--nointeractive set, --id must be set"))
            }
        }
    };

    let role = match permission.as_str() {
        "SERVER_ADMIN" => PermissionRole {
            role: Some(permission_role::Role::ServerAdmin(())),
        },
        "ORGANIZATION_ADMIN" => PermissionRole {
            role: Some(permission_role::Role::OrganizationAdmin(OrganizationRole {
                organization_id: get_id()?,
            })),
        },
        "ORGANIZATION_VIEWER" => PermissionRole {
            role: Some(permission_role::Role::OrganizationViewer(
                OrganizationRole {
                    organization_id: get_id()?,
                },
            )),
        },
        "EVENT_ADMIN" => PermissionRole {
            role: Some(permission_role::Role::EventAdmin(EventRole {
                event_id: get_id()?,
            })),
        },
        "EVENT_EDITOR" => PermissionRole {
            role: Some(permission_role::Role::EventEditor(EventRole {
                event_id: get_id()?,
            })),
        },
        "EVENT_VIEWER" => PermissionRole {
            role: Some(permission_role::Role::EventViewer(EventRole {
                event_id: get_id()?,
            })),
        },
        _ => {
            return Err(anyhow::anyhow!("Invalid permission"));
        }
    };

    permission_store
        .upsert(vec![Permission {
            id: "".to_owned(),
            user_id: user.id,
            role: Some(role),
        }])
        .await?;

    Ok(())
}

async fn init(
    email: Option<String>,
    password: Option<String>,
    username: Option<String>,
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
        let user_id = add_user(db.clone(), email, password, username, interactive).await?;

        let permission_store = PermissionStore::new(db.clone());
        permission_store
            .upsert(vec![Permission {
                id: "".to_owned(),
                user_id,
                role: Some(PermissionRole {
                    role: Some(permission_role::Role::ServerAdmin(())),
                }),
            }])
            .await?;
    }

    Ok(())
}

async fn login(
    username: Option<String>,
    password: Option<String>,
    interactive: bool,
) -> Result<(), anyhow::Error> {
    let db_url = db_url();
    let db = Arc::new(SqlitePool::connect(&db_url).await?);
    let user_store = UserStore::new(db.clone());
    let key_store = KeyStore::new(db.clone());
    let key_manager = StoreKeyManager::new(Arc::new(key_store));

    let username = match username {
        Some(username) => username,
        None => {
            if interactive {
                inquire::Text::new("Username").prompt()?
            } else {
                return Err(anyhow::anyhow!(
                    "--nointeractive set, --username must be set"
                ));
            }
        }
    };

    let password = match password {
        Some(password) => password,
        None => {
            if interactive {
                inquire::Password::new("Password")
                    .without_confirmation()
                    .prompt()?
            } else {
                return Err(anyhow::anyhow!(
                    "--nointeractive set, --password must be set"
                ));
            }
        }
    };

    let (_, token) = create_token(&key_manager, &user_store, username, password.as_str()).await?;
    println!("{}", token);

    Ok(())
}
