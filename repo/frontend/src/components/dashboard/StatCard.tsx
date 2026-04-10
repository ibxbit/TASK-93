import React from "react";
import styles from "./StatCard.module.css";

interface StatCardProps {
  label: string;
  value: string | number;
  icon?: string;
  accent?: "blue" | "green" | "amber" | "red";
}

export default function StatCard({
  label,
  value,
  icon,
  accent = "blue",
}: StatCardProps) {
  return (
    <div className={`${styles.card} ${styles[accent]}`}>
      {icon && <span className={styles.icon} aria-hidden>{icon}</span>}
      <span className={styles.value}>{value}</span>
      <span className={styles.label}>{label}</span>
    </div>
  );
}
