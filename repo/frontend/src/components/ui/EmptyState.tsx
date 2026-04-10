import React from "react";
import styles from "./EmptyState.module.css";

interface EmptyStateProps {
  title?: string;
  message?: string;
}

export default function EmptyState({
  title = "No records found",
  message = "Try adjusting your filters or search query.",
}: EmptyStateProps) {
  return (
    <div className={styles.wrapper} role="status">
      <span className={styles.icon} aria-hidden>📭</span>
      <h3 className={styles.title}>{title}</h3>
      <p className={styles.message}>{message}</p>
    </div>
  );
}
