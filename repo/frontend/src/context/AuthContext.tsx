import React, { createContext, useCallback, useContext, useEffect, useState } from "react";
import { authService } from "@/services/auth.service";
import { clearToken, getToken, getUser, saveToken, saveUser } from "@/utils/token";
import type { AuthUser, LoginRequest, Role } from "@/types";

interface AuthCtx {
  user: AuthUser | null;
  isLoading: boolean;
  login: (creds: LoginRequest) => Promise<void>;
  logout: () => Promise<void>;
  hasRole: (role: Role) => boolean;
}

export const AuthContext = createContext<AuthCtx | null>(null);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<AuthUser | null>(null);
  const [isLoading, setIsLoading] = useState(true);

  // Rehydrate from sessionStorage on mount
  useEffect(() => {
    const stored = getUser<AuthUser>();
    const token = getToken();
    if (stored && token) {
      setUser({ ...stored, token });
    }
    setIsLoading(false);
  }, []);

  const login = useCallback(async (creds: LoginRequest) => {
    const res = await authService.login(creds);
    // The API returns a token; we don't get roles from the login endpoint
    // so we infer them lazily. Roles are enforced server-side anyway.
    const authUser: AuthUser = {
      user_id: 0,
      username: creds.username,
      roles: [],
      token: res.token,
    };
    saveToken(res.token);
    saveUser(authUser);
    setUser(authUser);
  }, []);

  const logout = useCallback(async () => {
    try {
      await authService.logout();
    } finally {
      clearToken();
      setUser(null);
    }
  }, []);

  const hasRole = useCallback(
    (role: Role) => user?.roles.includes(role) ?? false,
    [user]
  );

  return (
    <AuthContext.Provider value={{ user, isLoading, login, logout, hasRole }}>
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth(): AuthCtx {
  const ctx = useContext(AuthContext);
  if (!ctx) throw new Error("useAuth must be used within AuthProvider");
  return ctx;
}
