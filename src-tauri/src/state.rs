use std::sync::{
    atomic::{AtomicBool, AtomicU32},
    Arc, Mutex, RwLock,
};

use crate::{
    ai::AiManager,
    hotkeys::{notify_menu_hotkey_changed, HotkeyTable},
    schema::LuairState,
    vault::{AccountProfile, AuthSession, LocalVault, VAULT_ENGINE_NAME},
};

pub struct AppState {
    pub state: Mutex<LuairState>,
    pub hotkey_table: Arc<RwLock<HotkeyTable>>,
    pub recording_hotkey: Arc<AtomicBool>,
    pub win_hotkey_tid: Arc<AtomicU32>,
    pub vault: Mutex<LocalVault>,
    pub active_account: Mutex<Option<AccountProfile>>,
    pub remember_me: Mutex<bool>,
    pub ai: Arc<AiManager>,
}

impl AppState {
    pub fn new(
        initial: LuairState,
        vault: LocalVault,
        active_account: Option<AccountProfile>,
        remember_me: bool,
        ai: Arc<AiManager>,
    ) -> Self {
        let table = HotkeyTable::from_state(&initial);
        Self {
            state: Mutex::new(initial),
            hotkey_table: Arc::new(RwLock::new(table)),
            recording_hotkey: Arc::new(AtomicBool::new(false)),
            win_hotkey_tid: Arc::new(AtomicU32::new(0)),
            vault: Mutex::new(vault),
            active_account: Mutex::new(active_account),
            remember_me: Mutex::new(remember_me),
            ai,
        }
    }

    pub fn snapshot(&self) -> LuairState {
        self.state
            .lock()
            .expect("AppState mutex poisoned")
            .clone()
    }

    pub fn sync_hotkeys(&self, state: &LuairState) {
        if let Ok(mut guard) = self.hotkey_table.write() {
            *guard = HotkeyTable::from_state(state);
        }
        notify_menu_hotkey_changed(&self.win_hotkey_tid);
    }

    pub fn set_account_session(
        &self,
        account: Option<AccountProfile>,
        remember: bool,
    ) -> Result<(), String> {
        {
            let mut acct_guard = self.active_account.lock().map_err(|e| e.to_string())?;
            *acct_guard = account;
        }
        {
            let mut rem_guard = self.remember_me.lock().map_err(|e| e.to_string())?;
            *rem_guard = remember;
        }
        Ok(())
    }

    pub fn auth_session(&self) -> AuthSession {
        let account = self
            .active_account
            .lock()
            .ok()
            .and_then(|guard| guard.clone());
        let remember_me = self
            .remember_me
            .lock()
            .map(|guard| *guard)
            .unwrap_or(false);
        let has_existing_accounts = self
            .vault
            .lock()
            .map(|v| v.has_existing_accounts())
            .unwrap_or(false);

        AuthSession {
            authenticated: account.is_some(),
            remember_me,
            account,
            has_existing_accounts,
            vault_engine: VAULT_ENGINE_NAME.to_string(),
        }
    }

    pub fn persist_active_account_state(&self, state: &LuairState) -> Result<(), String> {
        let account_id = self
            .active_account
            .lock()
            .map_err(|e| e.to_string())?
            .as_ref()
            .map(|a| a.id.clone());

        if let Some(id) = account_id {
            let vault = self.vault.lock().map_err(|e| e.to_string())?;
            vault.save_account_state(&id, state)?;
        }
        Ok(())
    }
}
