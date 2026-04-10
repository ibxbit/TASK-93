import React from "react";
import styles from "./Spinner.module.css";

interface SpinnerProps {
  label?: string;
}

export default function Spinner({ label = "Loading…" }: SpinnerProps) {
  return (
    <div className={styles.wrapper} role="status" aria-label={label}>
      <div className={styles.ring} />
      <span className={styles.label}>{label}</span>
    </div>
  );
}
