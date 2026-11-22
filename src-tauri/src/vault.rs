use std::fs;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use argon2::Argon2;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, State};

use crate::{
    cloak,
    schema::{LuairState, ThemeConfig},
    state::AppState,
    storage,
};

const VAULT_DB_FILE: &str = "luair-vault.db";
const VAULT_KEY_FILE: &str = "luair-vault.key";
pub const VAULT_ENGINE_NAME: &str = "SQLite + AES-256-GCM + Argon2id (Local)";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountProfile {
    pub id: String,
    pub username: String,
    pub email: String,
    pub created_at: i64,
    pub last_login_at: i64,
    pub onboarding_completed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSession {
    pub authenticated: bool,
    pub remember_me: bool,
    pub account: Option<AccountProfile>,
    pub has_existing_accounts: bool,
    pub vault_engine: String,
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    let mut buf = [0u8; N];
    getrandom::fill(&mut buf).map_err(|e| format!("CSPRNG error: {e}"))?;
    Ok(buf)
}

fn sha256_hex(input: &str) -> String {
    let digest = Sha256::digest(input.as_bytes());
    hex::encode(digest)
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (&x, &y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn hash_password(password: &str) -> Result<String, String> {
    let salt = random_bytes::<16>()?;
    let mut out = [0u8; 32];
    Argon2::default()
        .hash_password_into(password.as_bytes(), &salt, &mut out)
        .map_err(|e| format!("Password hashing failed: {e}"))?;
    Ok(format!("argon2id${}${}", hex::encode(salt), hex::encode(out)))
}

fn verify_password(password: &str, stored: &str) -> bool {
    let parts: Vec<&str> = stored.split('$').collect();
    if parts.len() != 3 || parts[0] != "argon2id" {
        return false;
    }
    let Ok(salt) = hex::decode(parts[1]) else {
        return false;
    };
    let Ok(expected) = hex::decode(parts[2]) else {
        return false;
    };
    let mut actual = vec![0u8; expected.len()];
    if Argon2::default()
        .hash_password_into(password.as_bytes(), &salt, &mut actual)
        .is_err()
    {
        return false;
    }
    constant_time_eq(&actual, &expected)
}

fn encrypt_text(key: &[u8; 32], plaintext: &str) -> Result<String, String> {
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("AES key error: {e}"))?;
    let nonce_bytes = random_bytes::<12>()?;
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| format!("AES-GCM encryption failed: {e}"))?;
    Ok(format!(
        "v1:{}:{}",
        hex::encode(nonce_bytes),
        hex::encode(ciphertext)
    ))
}

fn decrypt_text(key: &[u8; 32], payload: &str) -> Result<String, String> {
    let parts: Vec<&str> = payload.split(':').collect();
    if parts.len() != 3 || parts[0] != "v1" {
        return Err("Unsupported ciphertext format".to_string());
    }
    let nonce_bytes = hex::decode(parts[1]).map_err(|e| format!("Invalid nonce: {e}"))?;
    if nonce_bytes.len() != 12 {
        return Err("Invalid AES-GCM nonce length".to_string());
    }
    let ciphertext = hex::decode(parts[2]).map_err(|e| format!("Invalid ciphertext: {e}"))?;
    let cipher = Aes256Gcm::new_from_slice(key).map_err(|e| format!("AES key error: {e}"))?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce_bytes), ciphertext.as_slice())
        .map_err(|_| "AES-GCM decryption failed".to_string())?;
    String::from_utf8(plaintext).map_err(|e| format!("UTF-8 decode error: {e}"))
}

fn validate_username(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.len() < 3 || trimmed.len() > 24 {
        return Err("Username must be between 3 and 24 characters.".to_string());
    }
    if !trimmed
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(
            "Username may only contain letters, numbers, underscores, and hyphens.".to_string(),
        );
    }
    Ok(trimmed.to_string())
}

fn validate_email(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 120 {
        return Err("Please enter a valid email address.".to_string());
    }
    let parts: Vec<&str> = trimmed.split('@').collect();
    if parts.len() != 2 || parts[0].is_empty() || !parts[1].contains('.') {
        return Err("Please enter a valid email address.".to_string());
    }
    if parts[1].starts_with('.') || parts[1].ends_with('.') || trimmed.contains(char::is_whitespace) {
        return Err("Please enter a valid email address.".to_string());
    }
    Ok(trimmed.to_string())
}

fn validate_password(password: &str, confirm_password: &str) -> Result<(), String> {
    if password.len() < 8 {
        return Err("Password must be at least 8 characters long.".to_string());
    }
    if password != confirm_password {
        return Err("Passwords do not match.".to_string());
    }
    Ok(())
}

fn load_or_create_vault_key(dir: &Path) -> Result<[u8; 32], String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let key_path = dir.join(VAULT_KEY_FILE);
    if key_path.exists() {
        let raw = fs::read_to_string(&key_path).map_err(|e| e.to_string())?;
        let decoded = hex::decode(raw.trim()).map_err(|e| format!("Corrupt vault key: {e}"))?;
        if decoded.len() == 32 {
            let mut key = [0u8; 32];
            key.copy_from_slice(&decoded);
            return Ok(key);
        }
    }
    let key = random_bytes::<32>()?;
    fs::write(&key_path, hex::encode(key)).map_err(|e| e.to_string())?;
    Ok(key)
}

pub struct LocalVault {
    conn: Connection,
    key: [u8; 32],
}

impl LocalVault {
    pub fn open_at_dir(dir: &Path) -> Result<Self, String> {
        let key = load_or_create_vault_key(dir)?;
        let db_path = dir.join(VAULT_DB_FILE);
        let conn = Connection::open(&db_path).map_err(|e| e.to_string())?;
        let vault = Self { conn, key };
        vault.migrate()?;
        Ok(vault)
    }

    pub fn open_in_memory(key: [u8; 32]) -> Result<Self, String> {
        let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
        let vault = Self { conn, key };
        vault.migrate()?;
        Ok(vault)
    }

    fn migrate(&self) -> Result<(), String> {
        self.conn
            .execute_batch(
                "
                PRAGMA foreign_keys = ON;
                PRAGMA journal_mode = WAL;

                CREATE TABLE IF NOT EXISTS accounts (
                    id TEXT PRIMARY KEY,
                    username TEXT NOT NULL,
                    username_lower TEXT NOT NULL UNIQUE,
                    email_hash TEXT NOT NULL UNIQUE,
                    email_enc TEXT NOT NULL,
                    password_hash TEXT NOT NULL,
                    onboarding_completed INTEGER NOT NULL DEFAULT 0,
                    state_enc TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    last_login_at INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS sessions (
                    id INTEGER PRIMARY KEY CHECK (id = 1),
                    account_id TEXT NOT NULL,
                    token_enc TEXT NOT NULL,
                    token_hash TEXT NOT NULL,
                    remember_me INTEGER NOT NULL DEFAULT 1,
                    created_at INTEGER NOT NULL,
                    FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE CASCADE
                );
                ",
            )
            .map_err(|e| format!("SQLite migration failed: {e}"))
    }

    pub fn has_existing_accounts(&self) -> bool {
        self.conn
            .query_row("SELECT COUNT(*) FROM accounts", [], |row| row.get::<_, i64>(0))
            .map(|count| count > 0)
            .unwrap_or(false)
    }

    pub fn sign_up(
        &self,
        username: &str,
        email: &str,
        password: &str,
        confirm_password: &str,
        remember_me: bool,
        initial_state: &LuairState,
    ) -> Result<(AccountProfile, LuairState), String> {
        let clean_username = validate_username(username)?;
        let clean_email = validate_email(email)?;
        validate_password(password, confirm_password)?;

        let username_lower = clean_username.to_ascii_lowercase();
        let email_lower = clean_email.to_ascii_lowercase();
        let email_hash = sha256_hex(&email_lower);

        let dup_user: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM accounts WHERE username_lower = ?1",
                params![username_lower],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if dup_user.is_some() {
            return Err("That username is already registered in this local vault.".to_string());
        }

        let dup_email: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM accounts WHERE email_hash = ?1",
                params![email_hash],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| e.to_string())?;
        if dup_email.is_some() {
            return Err("That email is already registered in this local vault.".to_string());
        }

        let id = hex::encode(random_bytes::<16>()?);
        let password_hash = hash_password(password)?;
        let email_enc = encrypt_text(&self.key, &clean_email)?;
        let state_json = serde_json::to_string(initial_state).map_err(|e| e.to_string())?;
        let state_enc = encrypt_text(&self.key, &state_json)?;
        let ts = now_epoch();

        self.conn
            .execute(
                "INSERT INTO accounts (
                    id, username, username_lower, email_hash, email_enc,
                    password_hash, onboarding_completed, state_enc, created_at, last_login_at
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, ?7, ?8, ?9)",
                params![
                    id,
                    clean_username,
                    username_lower,
                    email_hash,
                    email_enc,
                    password_hash,
                    state_enc,
                    ts,
                    ts
                ],
            )
            .map_err(|e| e.to_string())?;

        if remember_me {
            self.write_session(&id, ts)?;
        } else {
            self.clear_session()?;
        }

        let profile = AccountProfile {
            id,
            username: clean_username,
            email: clean_email,
            created_at: ts,
            last_login_at: ts,
            onboarding_completed: false,
        };
        Ok((profile, initial_state.clone()))
    }

    pub fn sign_in(
        &self,
        identifier: &str,
        password: &str,
        remember_me: bool,
    ) -> Result<(AccountProfile, LuairState), String> {
        let ident = identifier.trim();
        if ident.is_empty() || password.is_empty() {
            return Err("Enter your username/email and password.".to_string());
        }

        let ident_lower = ident.to_ascii_lowercase();
        let ident_email_hash = sha256_hex(&ident_lower);

        let row = self
            .conn
            .query_row(
                "SELECT id, username, email_enc, password_hash, onboarding_completed, state_enc, created_at
                 FROM accounts
                 WHERE username_lower = ?1 OR email_hash = ?2
                 LIMIT 1",
                params![ident_lower, ident_email_hash],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, i64>(4)?,
                        r.get::<_, String>(5)?,
                        r.get::<_, i64>(6)?,
                    ))
                },
            )
            .optional()
            .map_err(|e| e.to_string())?;

        let Some((id, username, email_enc, password_hash, onboarding_int, state_enc, created_at)) =
            row
        else {
            return Err("Invalid local credentials.".to_string());
        };

        if !verify_password(password, &password_hash) {
            return Err("Invalid local credentials.".to_string());
        }

        let email = decrypt_text(&self.key, &email_enc)?;
        let state_json = decrypt_text(&self.key, &state_enc)?;
        let state: LuairState = serde_json::from_str(&state_json).unwrap_or_default();
        let ts = now_epoch();

        self.conn
            .execute(
                "UPDATE accounts SET last_login_at = ?1 WHERE id = ?2",
                params![ts, id],
            )
            .map_err(|e| e.to_string())?;

        if remember_me {
            self.write_session(&id, ts)?;
        } else {
            self.clear_session()?;
        }

        let profile = AccountProfile {
            id,
            username,
            email,
            created_at,
            last_login_at: ts,
            onboarding_completed: onboarding_int != 0,
        };
        Ok((profile, state))
    }

    pub fn restore_remembered_session(&self) -> Option<(AccountProfile, LuairState)> {
        let session_row = self
            .conn
            .query_row(
                "SELECT s.account_id, s.token_enc, s.token_hash,
                        a.username, a.email_enc, a.onboarding_completed, a.state_enc,
                        a.created_at, a.last_login_at
                 FROM sessions s
                 INNER JOIN accounts a ON a.id = s.account_id
                 WHERE s.id = 1 AND s.remember_me = 1",
                [],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, String>(4)?,
                        r.get::<_, i64>(5)?,
                        r.get::<_, String>(6)?,
                        r.get::<_, i64>(7)?,
                        r.get::<_, i64>(8)?,
                    ))
                },
            )
            .optional()
            .ok()??;

        let (
            account_id,
            token_enc,
            token_hash,
            username,
            email_enc,
            onboarding_int,
            state_enc,
            created_at,
            last_login_at,
        ) = session_row;

        let token = decrypt_text(&self.key, &token_enc).ok()?;
        if sha256_hex(&token) != token_hash {
            return None;
        }

        let email = decrypt_text(&self.key, &email_enc).ok()?;
        let state_json = decrypt_text(&self.key, &state_enc).ok()?;
        let state: LuairState = serde_json::from_str(&state_json).unwrap_or_default();

        Some((
            AccountProfile {
                id: account_id,
                username,
                email,
                created_at,
                last_login_at,
                onboarding_completed: onboarding_int != 0,
            },
            state,
        ))
    }

    pub fn save_account_state(&self, account_id: &str, state: &LuairState) -> Result<(), String> {
        let state_json = serde_json::to_string(state).map_err(|e| e.to_string())?;
        let state_enc = encrypt_text(&self.key, &state_json)?;
        self.conn
            .execute(
                "UPDATE accounts SET state_enc = ?1 WHERE id = ?2",
                params![state_enc, account_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn set_onboarding_completed(
        &self,
        account_id: &str,
        completed: bool,
        state: &LuairState,
    ) -> Result<(), String> {
        let state_json = serde_json::to_string(state).map_err(|e| e.to_string())?;
        let state_enc = encrypt_text(&self.key, &state_json)?;
        let flag = if completed { 1i64 } else { 0i64 };
        self.conn
            .execute(
                "UPDATE accounts SET onboarding_completed = ?1, state_enc = ?2 WHERE id = ?3",
                params![flag, state_enc, account_id],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    pub fn clear_session(&self) -> Result<(), String> {
        self.conn
            .execute("DELETE FROM sessions WHERE id = 1", [])
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn write_session(&self, account_id: &str, ts: i64) -> Result<(), String> {
        let token = hex::encode(random_bytes::<32>()?);
        let token_hash = sha256_hex(&token);
        let token_enc = encrypt_text(&self.key, &token)?;
        self.conn
            .execute(
                "INSERT INTO sessions (id, account_id, token_enc, token_hash, remember_me, created_at)
                 VALUES (1, ?1, ?2, ?3, 1, ?4)
                 ON CONFLICT(id) DO UPDATE SET
                   account_id = excluded.account_id,
                   token_enc = excluded.token_enc,
                   token_hash = excluded.token_hash,
                   remember_me = 1,
                   created_at = excluded.created_at",
                params![account_id, token_enc, token_hash, ts],
            )
            .map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn apply_authenticated_state(
    app: &AppHandle,
    app_state: &AppState,
    account: AccountProfile,
    remember_me: bool,
    next_state: LuairState,
) -> Result<AuthSession, String> {
    {
        let mut guard = app_state.state.lock().map_err(|e| e.to_string())?;
        *guard = next_state.clone();
    }
    app_state.sync_hotkeys(&next_state);
    app_state.set_account_session(Some(account), remember_me)?;

    let _ = cloak::apply_cloak_to_app(
        app,
        next_state.ui.streamproof,
        next_state.ui.cloak_menu,
    );
    let _ = storage::save_to_disk(app, &next_state);

    let session = app_state.auth_session();
    let _ = app.emit("luair://state-updated", &next_state);
    let _ = app.emit("luair://auth-updated", &session);
    Ok(session)
}

#[tauri::command]
pub fn get_auth_session(app_state: State<'_, AppState>) -> Result<AuthSession, String> {
    Ok(app_state.auth_session())
}

#[tauri::command]
pub fn sign_up_local(
    app: AppHandle,
    app_state: State<'_, AppState>,
    username: String,
    email: String,
    password: String,
    confirm_password: String,
    remember_me: bool,
) -> Result<AuthSession, String> {
    let initial_state = LuairState::default();
    let (account, account_state) = {
        let vault = app_state.vault.lock().map_err(|e| e.to_string())?;
        vault.sign_up(
            &username,
            &email,
            &password,
            &confirm_password,
            remember_me,
            &initial_state,
        )?
    };

    apply_authenticated_state(&app, &app_state, account, remember_me, account_state)
}

#[tauri::command]
pub fn sign_in_local(
    app: AppHandle,
    app_state: State<'_, AppState>,
    identifier: String,
    password: String,
    remember_me: bool,
) -> Result<AuthSession, String> {
    let (account, account_state) = {
        let vault = app_state.vault.lock().map_err(|e| e.to_string())?;
        vault.sign_in(&identifier, &password, remember_me)?
    };

    apply_authenticated_state(&app, &app_state, account, remember_me, account_state)
}

#[tauri::command]
pub fn sign_out_local(app: AppHandle, app_state: State<'_, AppState>) -> Result<AuthSession, String> {
    {
        let vault = app_state.vault.lock().map_err(|e| e.to_string())?;
        vault.clear_session()?;
    }
    app_state.set_account_session(None, false)?;
    let session = app_state.auth_session();
    let _ = app.emit("luair://auth-updated", &session);
    Ok(session)
}

#[tauri::command]
pub fn complete_onboarding(
    app: AppHandle,
    app_state: State<'_, AppState>,
    starter_profile_id: Option<String>,
    theme: Option<ThemeConfig>,
    enable_shield: Option<bool>,
    menu_hotkey: Option<String>,
) -> Result<AuthSession, String> {
    let next_state = {
        let mut guard = app_state.state.lock().map_err(|e| e.to_string())?;
        if let Some(pid) = starter_profile_id {
            if let Some(profile) = guard.profiles.iter().find(|p| p.id == pid).cloned() {
                guard.crosshair = profile.config;
                guard.active_profile_id = Some(profile.id);
            }
        }
        if let Some(t) = theme {
            guard.ui.ui_theme = t.name.clone();
            guard.ui.ui_accent = t.accent.clone();
            guard.ui.theme = t;
        }
        if let Some(shield) = enable_shield {
            guard.ui.streamproof = shield;
        }
        if let Some(hk) = menu_hotkey {
            let trimmed = hk.trim();
            if !trimmed.is_empty() {
                guard.ui.hotkey = trimmed.to_string();
            }
        }
        guard.clone()
    };

    app_state.sync_hotkeys(&next_state);
    let _ = cloak::apply_cloak_to_app(
        &app,
        next_state.ui.streamproof,
        next_state.ui.cloak_menu,
    );

    let updated_account = {
        let mut acct_guard = app_state.active_account.lock().map_err(|e| e.to_string())?;
        let account = acct_guard
            .as_mut()
            .ok_or_else(|| "No active local account session.".to_string())?;
        account.onboarding_completed = true;
        account.clone()
    };

    {
        let vault = app_state.vault.lock().map_err(|e| e.to_string())?;
        vault.set_onboarding_completed(&updated_account.id, true, &next_state)?;
    }

    let _ = storage::save_to_disk(&app, &next_state);
    let session = app_state.auth_session();
    let _ = app.emit("luair://state-updated", &next_state);
    let _ = app.emit("luair://auth-updated", &session);
    Ok(session)
}

#[tauri::command]
pub fn reopen_onboarding(
    app: AppHandle,
    app_state: State<'_, AppState>,
) -> Result<AuthSession, String> {
    let current_state = app_state.snapshot();
    let updated_account = {
        let mut acct_guard = app_state.active_account.lock().map_err(|e| e.to_string())?;
        let account = acct_guard
            .as_mut()
            .ok_or_else(|| "No active local account session.".to_string())?;
        account.onboarding_completed = false;
        account.clone()
    };

    {
        let vault = app_state.vault.lock().map_err(|e| e.to_string())?;
        vault.set_onboarding_completed(&updated_account.id, false, &current_state)?;
    }

    let session = app_state.auth_session();
    let _ = app.emit("luair://auth-updated", &session);
    Ok(session)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_vault() -> LocalVault {
        let key = [0x42u8; 32];
        LocalVault::open_in_memory(key).expect("open in-memory vault")
    }

    #[test]
    fn sign_up_persists_encrypted_email_and_salted_argon2_hash() {
        let vault = test_vault();
        let initial = LuairState::default();

        let (profile, restored) = vault
            .sign_up(
                "TacticalFox",
                "fox@luair.local",
                "SuperSecret99",
                "SuperSecret99",
                true,
                &initial,
            )
            .expect("sign up");

        assert_eq!(profile.username, "TacticalFox");
        assert_eq!(profile.email, "fox@luair.local");
        assert!(!profile.onboarding_completed);
        assert_eq!(restored, initial);

        let (stored_email_enc, stored_pw_hash, stored_state_enc): (String, String, String) =
            vault
                .conn
                .query_row(
                    "SELECT email_enc, password_hash, state_enc FROM accounts WHERE id = ?1",
                    params![profile.id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .expect("query raw account row");

        assert!(stored_email_enc.starts_with("v1:"));
        assert!(!stored_email_enc.contains("fox@luair.local"));
        assert!(stored_pw_hash.starts_with("argon2id$"));
        assert!(!stored_pw_hash.contains("SuperSecret99"));
        assert!(stored_state_enc.starts_with("v1:"));
        assert!(!stored_state_enc.contains("Primary (Rifle)"));
    }

    #[test]
    fn sign_in_works_by_username_or_email_and_rejects_wrong_password() {
        let vault = test_vault();
        let initial = LuairState::default();

        vault
            .sign_up(
                "ApexAim",
                "apex@example.com",
                "CorrectHorse42",
                "CorrectHorse42",
                false,
                &initial,
            )
            .expect("sign up");

        let (by_user, _) = vault
            .sign_in("apexaim", "CorrectHorse42", false)
            .expect("sign in by case-insensitive username");
        assert_eq!(by_user.username, "ApexAim");

        let (by_email, _) = vault
            .sign_in("APEX@example.com", "CorrectHorse42", true)
            .expect("sign in by case-insensitive email");
        assert_eq!(by_email.email, "apex@example.com");

        let bad = vault.sign_in("ApexAim", "WrongPass99", false);
        assert!(bad.is_err());
    }

    #[test]
    fn duplicate_username_and_email_and_mismatched_passwords_are_rejected() {
        let vault = test_vault();
        let initial = LuairState::default();

        let mismatch = vault.sign_up(
            "PlayerOne",
            "p1@example.com",
            "Password123",
            "Password456",
            true,
            &initial,
        );
        assert!(mismatch.is_err());

        vault
            .sign_up(
                "PlayerOne",
                "p1@example.com",
                "Password123",
                "Password123",
                true,
                &initial,
            )
            .expect("first user succeeds");

        let dup_user = vault.sign_up(
            "playerone",
            "other@example.com",
            "Password123",
            "Password123",
            true,
            &initial,
        );
        assert!(dup_user.is_err());

        let dup_email = vault.sign_up(
            "PlayerTwo",
            "P1@EXAMPLE.COM",
            "Password123",
            "Password123",
            true,
            &initial,
        );
        assert!(dup_email.is_err());
    }

    #[test]
    fn remember_me_session_restores_across_restart_and_sign_out_clears_it() {
        let vault = test_vault();
        let initial = LuairState::default();

        vault
            .sign_up(
                "TempUser",
                "temp@example.com",
                "LocalPass99",
                "LocalPass99",
                false,
                &initial,
            )
            .expect("sign up without remember_me");
        assert!(vault.restore_remembered_session().is_none());

        vault
            .sign_in("TempUser", "LocalPass99", true)
            .expect("sign in with remember_me");
        let (restored_acct, _) = vault
            .restore_remembered_session()
            .expect("remembered session restores");
        assert_eq!(restored_acct.username, "TempUser");

        vault.clear_session().expect("clear session");
        assert!(vault.restore_remembered_session().is_none());
    }

    #[test]
    fn per_account_encrypted_state_is_isolated_and_onboarding_flag_persists() {
        let vault = test_vault();
        let mut state_a = LuairState::default();
        state_a.crosshair.color = "#ff0055".to_string();

        let (acct_a, _) = vault
            .sign_up(
                "UserAlpha",
                "alpha@example.com",
                "AlphaPass88",
                "AlphaPass88",
                true,
                &state_a,
            )
            .expect("sign up alpha");

        let mut updated_a = state_a.clone();
        updated_a.crosshair.length = 28;
        vault
            .set_onboarding_completed(&acct_a.id, true, &updated_a)
            .expect("complete onboarding for alpha");

        let state_b = LuairState::default();
        vault
            .sign_up(
                "UserBeta",
                "beta@example.com",
                "BetaPass88",
                "BetaPass88",
                false,
                &state_b,
            )
            .expect("sign up beta");

        let (signed_a, loaded_a) = vault
            .sign_in("UserAlpha", "AlphaPass88", true)
            .expect("re-login alpha");
        assert!(signed_a.onboarding_completed);
        assert_eq!(loaded_a.crosshair.color, "#ff0055");
        assert_eq!(loaded_a.crosshair.length, 28);

        let (signed_b, loaded_b) = vault
            .sign_in("UserBeta", "BetaPass88", false)
            .expect("re-login beta");
        assert!(!signed_b.onboarding_completed);
        assert_eq!(loaded_b.crosshair.length, 10);
    }
}
