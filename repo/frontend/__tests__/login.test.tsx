/**
 * LoginForm — Happy Path & Failure Path
 *
 * Tests:
 *  1. Renders the form with username, password fields and submit button.
 *  2. Disables the submit button while submitting (loading state).
 *  3. Shows an inline error when the API returns 401 (bad credentials).
 *  4. Shows a generic error for non-401 network failures.
 *  5. Calls onSuccess when login resolves successfully.
 */

import React from "react";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import LoginForm from "@/components/auth/LoginForm";
import { AuthContext } from "@/context/AuthContext";
import { ApiRequestError } from "@/services/api";
import type { ApiError, LoginRequest } from "@/types";

// ── Helper: build a minimal AuthContext value ──────────────────────────────────
function makeAuthCtx(loginImpl: (creds: LoginRequest) => Promise<void>) {
  return {
    user: null,
    isLoading: false,
    login: loginImpl,
    logout: jest.fn<Promise<void>, []>().mockResolvedValue(undefined),
    hasRole: jest.fn().mockReturnValue(false),
  };
}

function renderForm(
  loginImpl: (creds: LoginRequest) => Promise<void>,
  onSuccess = jest.fn()
) {
  return render(
    <AuthContext.Provider value={makeAuthCtx(loginImpl)}>
      <LoginForm onSuccess={onSuccess} />
    </AuthContext.Provider>
  );
}

// ── Tests ──────────────────────────────────────────────────────────────────────

describe("LoginForm", () => {
  it("renders username, password inputs and a submit button", () => {
    renderForm(jest.fn<Promise<void>, [LoginRequest]>().mockResolvedValue(undefined));

    expect(screen.getByLabelText(/username/i)).toBeInTheDocument();
    expect(screen.getByLabelText(/password/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /sign in/i })).toBeInTheDocument();
  });

  it("submit button is disabled when fields are empty", () => {
    renderForm(jest.fn<Promise<void>, [LoginRequest]>().mockResolvedValue(undefined));
    expect(screen.getByRole("button", { name: /sign in/i })).toBeDisabled();
  });

  it("calls onSuccess and does NOT show error on successful login", async () => {
    const onSuccess = jest.fn();
    const login = jest.fn<Promise<void>, [LoginRequest]>().mockResolvedValue(undefined);

    renderForm(login, onSuccess);

    await userEvent.type(screen.getByLabelText(/username/i), "admin");
    await userEvent.type(screen.getByLabelText(/password/i), "secret");
    fireEvent.click(screen.getByRole("button", { name: /sign in/i }));

    await waitFor(() => expect(onSuccess).toHaveBeenCalledTimes(1));
    expect(screen.queryByTestId("login-error")).not.toBeInTheDocument();
  });

  it("shows 'Invalid username or password' error on 401", async () => {
    const apiError: ApiError = { code: "UNAUTHORIZED", message: "Bad credentials" };
    const login = jest
      .fn()
      .mockRejectedValue(new ApiRequestError(401, apiError));

    renderForm(login);

    await userEvent.type(screen.getByLabelText(/username/i), "admin");
    await userEvent.type(screen.getByLabelText(/password/i), "wrong");
    fireEvent.click(screen.getByRole("button", { name: /sign in/i }));

    await waitFor(() =>
      expect(screen.getByTestId("login-error")).toHaveTextContent(
        /invalid username or password/i
      )
    );
  });

  it("shows a generic error message for non-401 failures", async () => {
    const login = jest
      .fn()
      .mockRejectedValue(new Error("Network timeout"));

    renderForm(login);

    await userEvent.type(screen.getByLabelText(/username/i), "admin");
    await userEvent.type(screen.getByLabelText(/password/i), "pass");
    fireEvent.click(screen.getByRole("button", { name: /sign in/i }));

    await waitFor(() =>
      expect(screen.getByTestId("login-error")).toHaveTextContent(
        /network timeout/i
      )
    );
  });

  it("disables the submit button and shows 'Signing in…' while submitting", async () => {
    let resolve!: () => void;
    const login = jest.fn(
      () =>
        new Promise<void>((res) => {
          resolve = res;
        })
    );

    renderForm(login);

    await userEvent.type(screen.getByLabelText(/username/i), "admin");
    await userEvent.type(screen.getByLabelText(/password/i), "pass");
    fireEvent.click(screen.getByRole("button", { name: /sign in/i }));

    // While the promise is pending the button should be disabled
    await waitFor(() =>
      expect(screen.getByRole("button", { name: /signing in/i })).toBeDisabled()
    );

    // Resolve so no state-update warning fires after the test
    resolve();
  });
});
