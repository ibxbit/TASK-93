import React from "react";
import styles from "./ErrorBanner.module.css";

interface ErrorBannerProps {
  message: string;
  onRetry?: () => void;
}

export default function ErrorBanner({ message, onRetry }: ErrorBannerProps) {
  return (
    <div className={styles.banner} role="alert">
      <span className={styles.icon} aria-hidden>⚠️</span>
      <span className={styles.message}>{message}</span>
      {onRetry && (
        <button className={styles.retry} onClick={onRetry} type="button">
          Retry
        </button>
      )}
    </div>
  );
}
