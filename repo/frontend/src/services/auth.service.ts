import { api } from "./api";
import type { LoginRequest, LoginResponse } from "@/types";

export const authService = {
  login(credentials: LoginRequest): Promise<LoginResponse> {
    return api.post<LoginResponse>("/auth/login", credentials);
  },

  logout(): Promise<void> {
    return api.post<void>("/auth/logout", {});
  },
};
