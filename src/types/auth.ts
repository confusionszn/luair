export interface AccountProfile {
  id: string;
  username: string;
  email: string;
  createdAt: number;
  lastLoginAt: number;
  onboardingCompleted: boolean;
}

export interface AuthSession {
  authenticated: boolean;
  rememberMe: boolean;
  account: AccountProfile | null;
  hasExistingAccounts: boolean;
  vaultEngine: string;
}

export const DEFAULT_AUTH_SESSION: AuthSession = {
  authenticated: false,
  rememberMe: false,
  account: null,
  hasExistingAccounts: false,
  vaultEngine: "SQLite + AES-256-GCM + Argon2id (Local)",
};
