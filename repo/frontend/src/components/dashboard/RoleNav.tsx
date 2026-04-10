import React from "react";
import Link from "next/link";
import type { Role } from "@/types";
import { labelRole } from "@/utils/format";
import styles from "./RoleNav.module.css";

interface NavItem {
  href: string;
  label: string;
  roles: Role[]; // empty = visible to all authenticated users
  description: string;
}

const NAV_ITEMS: NavItem[] = [
  {
    href: "/assets",
    label: "Asset Register",
    roles: [],
    description: "View and manage fixed assets with depreciation tracking.",
  },
  {
    href: "/results",
    label: "Events & Results",
    roles: ["Administrator", "EventDirector", "Referee"],
    description: "Browse motorsport events and per-event rankings.",
  },
  {
    href: "/invoices",
    label: "Invoices",
    roles: ["Administrator", "FinanceClerk", "Auditor"],
    description: "Manage invoices, line items, and payments.",
  },
  {
    href: "/audit",
    label: "Audit Logs",
    roles: ["Administrator", "Auditor"],
    description: "Immutable, append-only activity trail.",
  },
  {
    href: "/admin",
    label: "Administration",
    roles: ["Administrator"],
    description: "Manage user roles and system backups.",
  },
];

interface RoleNavProps {
  userRoles: Role[];
}

export default function RoleNav({ userRoles }: RoleNavProps) {
  const visible = NAV_ITEMS.filter(
    (item) =>
      item.roles.length === 0 ||
      item.roles.some((r) => userRoles.includes(r))
  );

  return (
    <section>
      <h2 className={styles.heading}>Quick Navigation</h2>
      {userRoles.length > 0 && (
        <p className={styles.rolePills}>
          Your roles:{" "}
          {userRoles.map((r) => (
            <span key={r} className={styles.pill}>
              {labelRole(r)}
            </span>
          ))}
        </p>
      )}
      <ul className={styles.grid}>
        {visible.map((item) => (
          <li key={item.href}>
            <Link href={item.href} className={styles.card}>
              <span className={styles.cardLabel}>{item.label}</span>
              <span className={styles.cardDesc}>{item.description}</span>
            </Link>
          </li>
        ))}
      </ul>
    </section>
  );
}
