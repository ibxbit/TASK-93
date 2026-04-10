import React from "react";
import Link from "next/link";
import { useRouter } from "next/router";
import { useAuth } from "@/context/AuthContext";
import { useToast } from "@/context/ToastContext";
import styles from "./Navbar.module.css";

const NAV_LINKS = [
  { href: "/dashboard", label: "Dashboard" },
  { href: "/assets", label: "Assets" },
  { href: "/results", label: "Results" },
  { href: "/invoices", label: "Invoices" },
];

export default function Navbar() {
  const { user, logout } = useAuth();
  const { addToast } = useToast();
  const router = useRouter();

  async function handleLogout() {
    try {
      await logout();
      router.push("/login");
    } catch {
      addToast("Logout failed. Please try again.", "error");
    }
  }

  return (
    <nav className={styles.nav}>
      <span className={styles.brand}>🏎 Motorsport Ops</span>
      <ul className={styles.links}>
        {NAV_LINKS.map(({ href, label }) => (
          <li key={href}>
            <Link
              href={href}
              className={router.pathname === href ? styles.active : undefined}
            >
              {label}
            </Link>
          </li>
        ))}
      </ul>
      <div className={styles.user}>
        {user && (
          <>
            <span className={styles.username}>{user.username}</span>
            <button
              type="button"
              className={styles.logoutBtn}
              onClick={handleLogout}
            >
              Sign out
            </button>
          </>
        )}
      </div>
    </nav>
  );
}
