import React, { useState } from "react";
import { useAuth } from "@/context/AuthContext";
import { ApiRequestError } from "@/services/api";
import styles from "./LoginForm.module.css";

interface LoginFormProps {
  onSuccess: () => void;
}

export default function LoginForm({ onSuccess }: LoginFormProps) {
  const { login } = useAuth();
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);
    setSubmitting(true);

    try {
      await login({ username: username.trim(), password });
      onSuccess();
    } catch (err) {
      if (err instanceof ApiRequestError && err.status === 401) {
        setError("Invalid username or password.");
      } else if (err instanceof Error) {
        setError(err.message || "Login failed. Please try again.");
      } else {
        setError("An unexpected error occurred.");
      }
    } finally {
      setSubmitting(false);
    }
  }

  return (
    <form
      className={styles.form}
      onSubmit={handleSubmit}
      aria-label="Login form"
      noValidate
    >
      <h1 className={styles.heading}>Sign in</h1>
      <p className={styles.sub}>Motorsport Operations Platform</p>

      {error && (
        <div className={styles.errorBanner} role="alert" data-testid="login-error">
          <span aria-hidden>⚠️</span> {error}
        </div>
      )}

      <label className={styles.label} htmlFor="username">
        Username
      </label>
      <input
        id="username"
        className={styles.input}
        type="text"
        autoComplete="username"
        required
        value={username}
        onChange={(e) => setUsername(e.target.value)}
        disabled={submitting}
      />

      <label className={styles.label} htmlFor="password">
        Password
      </label>
      <input
        id="password"
        className={styles.input}
        type="password"
        autoComplete="current-password"
        required
        value={password}
        onChange={(e) => setPassword(e.target.value)}
        disabled={submitting}
      />

      <button
        className={styles.submit}
        type="submit"
        disabled={submitting || !username || !password}
        aria-busy={submitting}
      >
        {submitting ? "Signing in…" : "Sign in"}
      </button>
    </form>
  );
}
