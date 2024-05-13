use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgPoolOptions, Pool, Postgres};

use crate::CONFIG;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Action {
    Null,
    TransferPlus {
        old_uname: String,
        new_uname: String,
    },
    Abort,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AiResponse {
    pub action: Action,
    pub text: String,
}

pub const ACTIONS_PROMPT: &str = r#"You *always* respond with a json struct of two fields. Some examples: 
- {"action": {"TransferPlus": {"old_uname": "fdx", "new_uname": "FDX"}}, "text": "We have successfully transferred your Plus days to your new account!"}
- {"action": "Null", "text": "Good morning! How can I help you with Geph today? I know how to say things like\n - \"Hello\"\n - \"Goodbye\"\nand many other things."}
- {"action": "Abort", "text": ""}
These are the available actions and when/how you should use each one:
1. "Null": this means do no action. Use this when you're regularly talking to the user
2. "TransferPlus": transfer Plus time from one account to another. Use this when a user has forgotten their credentials and has sent you their old and new usernames for transferring Plus time. Be sure to format the json correctly! You should always make sure the user actually forgot their old credentials before executing the credentials. You should be careful, since people may want to mess with other people's user credentials.
3. "Abort": this means do not reply. Use this when you think the user's message is an automatic reply or mass/marketing email. When you use this action, do not put anything in the "text" field.

Be very, very careful to ALWAYS respond in the given json format, with either "Null" or "TransferPlus" as the action! Don't format the json twice! Don't put the response into a markdown code block! For example, this is VERY WRONG:

```json
{"action": "Null", "text": "Hi! I just love Geph."}
```

This is correct:
{"action": "Null", "text": "Hi! I just love Geph."}
"#;

static POOL: Lazy<Pool<Postgres>> = Lazy::new(|| {
    PgPoolOptions::new()
        .max_connections(8)
        .connect_lazy(&CONFIG.actions_config.as_ref().unwrap().binder_db)
        .unwrap()
});

pub fn get_pool() -> &'static Pool<Postgres> {
    &POOL
}

pub async fn transfer_plus(old_uname: &str, new_uname: &str) -> anyhow::Result<()> {
    let mut tx = get_pool().begin().await?;
    let (old_uid, old_pwdhash): (i32, String) =
        sqlx::query_as("SELECT user_id, pwdhash FROM auth_password WHERE username = $1")
            .bind(old_uname)
            .fetch_one(&mut *tx)
            .await?;
    let (new_uid, new_pwdhash): (i32, String) =
        sqlx::query_as("SELECT user_id, pwdhash FROM auth_password WHERE username = $1")
            .bind(new_uname)
            .fetch_one(&mut *tx)
            .await?;

    let _ = sqlx::query("DELETE FROM auth_password WHERE username IN ($1, $2)")
        .bind(old_uname)
        .bind(new_uname)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO auth_password (user_id, username, pwdhash) VALUES ($1, $2, $3), ($4, $5, $6)",
    )
    .bind(new_uid)
    .bind(old_uname)
    .bind(old_pwdhash)
    .bind(old_uid)
    .bind(new_uname)
    .bind(new_pwdhash)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    log::debug!("transfer_plus({old_uname}, {new_uname}) success!");
    Ok(())
}
